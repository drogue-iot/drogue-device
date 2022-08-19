#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use core::future::Future;
use drogue_device::bsp::boards::nrf52::microbit::*;
use drogue_device::domain::led::matrix::Frame;
use drogue_device::drivers::ble::mesh::bearer::nrf52::{
    Nrf52BleMeshFacilities, SoftdeviceAdvertisingBearer, SoftdeviceRng,
};
use drogue_device::drivers::ble::mesh::composition::{
    CompanyIdentifier, Composition, ElementDescriptor, ElementsHandler, Features, Location,
    ProductIdentifier, VersionIdentifier,
};
use drogue_device::drivers::ble::mesh::config::ConfigurationModel;
use drogue_device::drivers::ble::mesh::driver::elements::AppElementsContext;
use drogue_device::drivers::ble::mesh::driver::DeviceError;
use drogue_device::drivers::ble::mesh::interface::AdvertisingOnlyNetworkInterfaces;
use drogue_device::drivers::ble::mesh::model::ModelIdentifier;
use drogue_device::drivers::ble::mesh::pdu::access::AccessMessage;
use drogue_device::drivers::ble::mesh::pdu::ParseError;
use drogue_device::drivers::ble::mesh::provisioning::{
    Algorithms, Capabilities, InputOOBActions, OOBSize, OutputOOBActions, PublicKeyType,
    StaticOOBType,
};
use drogue_device::drivers::ble::mesh::storage::FlashStorage;
use drogue_device::drivers::ble::mesh::InsufficientBuffer;
use drogue_device::{
    actors::ble::mesh::{MeshNode, MeshNodeMessage},
    drivers::ble::mesh::model::sensor::{
        PropertyId, SensorConfig, SensorData, SensorDescriptor, SensorMessage, SensorServer,
        SensorStatus, SENSOR_SERVER,
    },
    flash::{FlashState, SharedFlash},
    Board,
};

cfg_if::cfg_if! {
    if #[cfg(feature = "dfu")] {
        use drogue_device::{firmware::FirmwareManager, drivers::ble::mesh::model::{Model, firmware::{
            Control as FirmwareControl, FirmwareUpdateMessage, FirmwareUpdateServer,
            Status as FirmwareStatus, FIRMWARE_UPDATE_SERVER,
            drivers::ble::mesh::model::Model,
            firmware::FirmwareManager,
        }}};
        use embassy_boot_nrf::FirmwareUpdater;
    }
}

use embassy_util::channel::mpmc::{Channel, DynamicReceiver, DynamicSender};
use embassy_time::Ticker;
use embassy_time::{Duration, Timer};
use embassy_util::Forever;
use embassy_util::{select, Either};
use embassy_util::blocking_mutex::raw::NoopRawMutex;
use embassy_executor::Spawner;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::Peripherals;
use futures::StreamExt;
use heapless::Vec;
use nrf_softdevice::{temperature_celsius, Softdevice};

use nrf_softdevice::Flash;

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

#[cfg(feature = "panic-reset")]
use panic_reset as _;

type ConcreteMeshNode = MeshNode<
    'static,
    CustomElementsHandler,
    AdvertisingOnlyNetworkInterfaces<SoftdeviceAdvertisingBearer>,
    FlashStorage<SharedFlash<'static, Flash>>,
    SoftdeviceRng,
>;

pub struct MyDevice {
    control: Channel<NoopRawMutex, MeshNodeMessage, 1>,
    publisher: Channel<NoopRawMutex, PublisherMessage, 1>,
    state: Channel<NoopRawMutex, AppState, 1>,
}

static DEVICE: Forever<MyDevice> = Forever::new();
static MESH: Forever<ConcreteMeshNode> = Forever::new();

// Application must run at a lower priority than softdevice
fn config() -> Config {
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}

extern "C" {
    static __storage: u8;
}

const COMPANY_IDENTIFIER: CompanyIdentifier = CompanyIdentifier(0x0003);
const PRODUCT_IDENTIFIER: ProductIdentifier = ProductIdentifier(0x0001);
const VERSION_IDENTIFIER: VersionIdentifier = VersionIdentifier(0x0001);
const FEATURES: Features = Features {
    relay: true,
    proxy: false,
    friend: false,
    low_power: false,
};

const FIRMWARE_VERSION: &str = env!("CARGO_PKG_VERSION");
const FIRMWARE_REVISION: Option<&str> = option_env!("REVISION");

#[embassy_executor::main(config = "config()")]
async fn main(spawner: Spawner, p: Peripherals) {
    let board = Microbit::new(p);
    let facilities = Nrf52BleMeshFacilities::new("Drogue IoT BT Mesh", false);
    spawner.spawn(softdevice_task(facilities.sd())).unwrap();

    static FLASH: FlashState<Flash> = FlashState::new();
    let flash = FLASH.initialize(facilities.flash());
    let advertising_bearer = facilities.advertising_bearer();
    //let gatt_bearer = facilities.gatt_bearer();
    let rng = facilities.rng();
    let storage = FlashStorage::new(unsafe { &__storage as *const u8 as usize }, flash.clone());

    let capabilities = Capabilities {
        #[cfg(feature = "dfu")]
        number_of_elements: 2,
        #[cfg(not(feature = "dfu"))]
        number_of_elements: 1,
        algorithms: Algorithms::default(),
        public_key_type: PublicKeyType::default(),
        static_oob_type: StaticOOBType::default(),
        output_oob_size: OOBSize::MaximumSize(4),
        output_oob_action: OutputOOBActions::default(),
        input_oob_size: OOBSize::MaximumSize(4),
        input_oob_action: InputOOBActions::default(),
    };

    let device = DEVICE.put(MyDevice {
        publisher: Channel::new(),
        control: Channel::new(),
        state: Channel::new(),
    });

    let mut composition = Composition::new(
        COMPANY_IDENTIFIER,
        PRODUCT_IDENTIFIER,
        VERSION_IDENTIFIER,
        FEATURES,
    );
    composition
        .add_element(ElementDescriptor::new(Location(0x0001)).add_model(SENSOR_SERVER))
        .ok();
    #[cfg(feature = "dfu")]
    composition
        .add_element(ElementDescriptor::new(Location(0x0002)).add_model(FIRMWARE_UPDATE_SERVER))
        .ok();

    let version = FIRMWARE_REVISION.unwrap_or(FIRMWARE_VERSION);
    defmt::info!("Running firmware version {}", version);
    #[cfg(feature = "dfu")]
    let dfu = FirmwareManager::new(flash, FirmwareUpdater::default(), version.as_bytes());

    let elements = CustomElementsHandler {
        display: device.state.sender().into(),
        ctx: None,
        #[cfg(feature = "dfu")]
        fw_state: MeshFirmwareState {
            next_offset: 0,
            next_version: Vec::from_slice(version.as_bytes()).unwrap(),
        },
        #[cfg(feature = "dfu")]
        dfu,
        composition,
        publisher: device.publisher.sender().into(),
    };
    let network = AdvertisingOnlyNetworkInterfaces::new(advertising_bearer);
    defmt::info!(
        "Mesh node size: {}",
        core::mem::size_of::<ConcreteMeshNode>()
    );
    let mesh_node = MESH.put(MeshNode::new(elements, capabilities, network, storage, rng));
    spawner
        .spawn(mesh_task(mesh_node, device.control.receiver().into()))
        .unwrap();

    spawner
        .spawn(reset_task(
            ButtonA::new(board.btn_a),
            device.control.sender().into(),
        ))
        .unwrap();

    spawner
        .spawn(publisher_task(
            Duration::from_secs(10),
            facilities.sd(),
            device.publisher.receiver().into(),
        ))
        .unwrap();

    spawner.spawn(watchdog_task()).unwrap();
}

#[embassy_executor::task]
async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}

#[embassy_executor::task]
pub async fn mesh_task(
    node: &'static mut ConcreteMeshNode,
    control: DynamicReceiver<'static, MeshNodeMessage>,
) {
    node.run(control).await;
}

#[embassy_executor::task]
pub async fn reset_task(mut button: ButtonA, control: DynamicSender<'static, MeshNodeMessage>) {
    loop {
        button.wait_released().await;
        control.send(MeshNodeMessage::ForceReset).await;
    }
}

#[derive(Clone, Copy)]
pub enum AppState {
    Unprovisioned,
    Provisioned,
    Connected,
}

impl Into<Frame<5, 5>> for AppState {
    fn into(self) -> Frame<5, 5> {
        match self {
            AppState::Unprovisioned => animation::UNPROVISIONED,
            AppState::Provisioned => animation::PROVISIONED,
            AppState::Connected => animation::CONNECTED,
        }
    }
}

mod animation {
    use drogue_device::domain::led::matrix::Frame;
    use drogue_device::drivers::led::matrix::fonts::frame_5x5;
    use drogue_device::drivers::led::matrix::fonts::CHECK_MARK;
    use drogue_device::drivers::led::matrix::fonts::CROSS_MARK;

    #[rustfmt::skip]
    pub const P: &[u8; 5] = &[
        0b01110,
        0b01001,
        0b01110,
        0b01010,
        0b01000,
    ];

    pub const UNPROVISIONED: Frame<5, 5> = frame_5x5(CROSS_MARK);
    pub const PROVISIONED: Frame<5, 5> = frame_5x5(P);
    pub const CONNECTED: Frame<5, 5> = frame_5x5(CHECK_MARK);
}

#[embassy_executor::task]
async fn state_task(mut display: LedMatrix, inbox: DynamicReceiver<'static, AppState>) {
    let mut ticker = Ticker::every(Duration::from_secs(5));
    let mut current = AppState::Unprovisioned;
    loop {
        match select(ticker.next(), inbox.recv()).await {
            Either::First(_) => {
                display
                    .display(current.into(), Duration::from_secs(1))
                    .await;
            }
            Either::Second(s) => {
                current = s;
            }
        }
    }
}

#[embassy_executor::task]
async fn publisher_task(
    interval: Duration,
    sd: &'static Softdevice,
    inbox: DynamicReceiver<'static, PublisherMessage>,
) {
    let mut context = None;
    let mut ticker = Ticker::every(interval);
    loop {
        let next = inbox.recv();
        let tick = ticker.next();

        match select(next, tick).await {
            Either::First(message) => match message {
                PublisherMessage::Connect(ctx) => {
                    context.replace(ctx);
                }
                PublisherMessage::SetPeriod(interval) => {
                    ticker = Ticker::every(interval);
                }
            },
            Either::Second(_) => {
                let value: i8 = temperature_celsius(sd).unwrap().to_num();
                defmt::info!("Measured temperature: {}℃", value);
                let value = value * 2;
                if let Some(ctx) = &context {
                    // Report sensor data
                    let c = ctx.for_element_model::<SensorServer<SensorModel, 1, 1>>(0);
                    let message = SensorMessage::Status(SensorStatus::new(Temperature(value)));
                    match c.publish(message).await {
                        Ok(_) => {
                            defmt::debug!("Published sensor data");
                        }
                        Err(e) => {
                            defmt::warn!("Error reporting sensor data: {:?}", e);
                        }
                    }
                } else {
                    defmt::info!("Read sensor values: {:?}", value);
                }
            }
        }
    }
}

// Keeps our system alive
#[embassy_executor::task]
async fn watchdog_task() {
    let mut handle = unsafe { embassy_nrf::wdt::WatchdogHandle::steal(0) };
    loop {
        handle.pet();
        Timer::after(Duration::from_secs(2)).await;
    }
}

#[allow(unused)]
pub struct CustomElementsHandler {
    display: DynamicSender<'static, AppState>,
    composition: Composition,
    #[cfg(feature = "dfu")]
    dfu: FirmwareManager<SharedFlash<'static, Flash>>,
    publisher: DynamicSender<'static, PublisherMessage>,
    #[cfg(feature = "dfu")]
    fw_state: MeshFirmwareState,
    ctx: Option<AppElementsContext<'static>>,
}

pub enum PublisherMessage {
    Connect(AppElementsContext<'static>),
    SetPeriod(Duration),
}

#[cfg(feature = "dfu")]
pub struct MeshFirmwareState {
    next_version: Vec<u8, 16>,
    next_offset: u32,
}

impl ElementsHandler<'static> for CustomElementsHandler {
    fn composition(&self) -> &Composition {
        &self.composition
    }

    fn connect(&mut self, ctx: AppElementsContext<'static>) {
        let _ = self
            .publisher
            .try_send(PublisherMessage::Connect(ctx.clone()));
        let _ = self.display.try_send(AppState::Connected);
        self.ctx.replace(ctx);
    }

    fn configure(&mut self, config: &ConfigurationModel) {
        if let Some(period) = config.publish_period_duration() {
            let _ = self.publisher.try_send(PublisherMessage::SetPeriod(period));
        }
    }

    type DispatchFuture<'m> = impl Future<Output = Result<(), DeviceError>> + 'm where Self: 'm;
    fn dispatch<'m>(
        &'m mut self,
        element: u8,
        model_identifier: &'m ModelIdentifier,
        access: &'m AccessMessage,
    ) -> Self::DispatchFuture<'m> {
        async move {
            defmt::debug!(
                "Received access message for element {}, model {:?}. Opcode 0x{:x}, Param len: {:?}",
                element,
                model_identifier,
                access.opcode(),
                access.parameters().len()
            );
            #[cfg(feature = "dfu")]
            if element == 1 && *model_identifier == FIRMWARE_UPDATE_SERVER {
                match FirmwareUpdateServer::parse(access.opcode(), access.parameters()) {
                    Ok(Some(message)) => {
                        defmt::info!("Received firmware message: {:?}", message);
                        match message {
                            FirmwareUpdateMessage::Get => {
                                if let Some(ctx) = &self.ctx {
                                    let status = FirmwareUpdateMessage::Status(FirmwareStatus {
                                        mtu: 16,
                                        offset: self.fw_state.next_offset,
                                        version: &self.fw_state.next_version,
                                    });

                                    match ctx.respond(access, status).await {
                                        Ok(_) => {
                                            defmt::debug!("Sent status response");
                                        }
                                        Err(e) => {
                                            defmt::warn!("Error reporting status: {:?}", e);
                                        }
                                    }
                                }
                            }
                            FirmwareUpdateMessage::Control(control) => match control {
                                FirmwareControl::Start => {
                                    self.fw_state.next_offset = 0;
                                    if let Err(e) =
                                        self.dfu.start(&self.fw_state.next_version).await
                                    {
                                        defmt::warn!(
                                            "Error starting DFU: {:?}",
                                            defmt::Debug2Format(&e)
                                        );
                                    }
                                }
                                FirmwareControl::Update => {
                                    if let Err(e) =
                                        self.dfu.update(&self.fw_state.next_version, &[]).await
                                    {
                                        defmt::warn!(
                                            "Error marking firmware to be swapped: {:?}",
                                            defmt::Debug2Format(&e)
                                        );
                                    }
                                }
                                FirmwareControl::NextVersion(version) => {
                                    if let Ok(v) = Vec::from_slice(version) {
                                        self.fw_state.next_version = v;
                                    }
                                }
                                FirmwareControl::MarkBooted => {
                                    if let Err(e) = self.dfu.synced().await {
                                        defmt::warn!(
                                            "Error marking firmware as good: {:?}",
                                            defmt::Debug2Format(&e)
                                        );
                                    }
                                }
                            },
                            FirmwareUpdateMessage::Write(write) => {
                                if write.offset != self.fw_state.next_offset {
                                    defmt::warn!(
                                        "Unexpected write at offset {}, was expecting {}",
                                        write.offset,
                                        self.fw_state.next_offset
                                    );
                                } else {
                                    if let Err(e) =
                                        self.dfu.write(write.offset, write.payload).await
                                    {
                                        defmt::warn!(
                                            "Error writing {} bytes at offset {}: {:?}",
                                            write.payload.len(),
                                            write.offset,
                                            defmt::Debug2Format(&e),
                                        );
                                    } else {
                                        self.fw_state.next_offset += write.payload.len() as u32;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    Ok(None) => {
                        defmt::info!("No parseable message!");
                    }
                    Err(e) => {
                        defmt::warn!("Error parsing firmware update message: {:?}", e);
                    }
                }
            }
            Ok(())
        }
    }
}

#[derive(Clone, defmt::Format)]
pub struct SensorModel;

#[derive(Clone, defmt::Format)]
pub struct Temperature(i8);

impl SensorConfig for SensorModel {
    type Data<'m> = Temperature;

    const DESCRIPTORS: &'static [SensorDescriptor] = &[SensorDescriptor::new(PropertyId(0x4F), 1)];
}

impl SensorData for Temperature {
    fn decode(&mut self, _: PropertyId, _: &[u8]) -> Result<(), ParseError> {
        todo!()
    }

    fn encode<const N: usize>(
        &self,
        _: PropertyId,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.extend_from_slice(&self.0.to_le_bytes())
            .map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}
