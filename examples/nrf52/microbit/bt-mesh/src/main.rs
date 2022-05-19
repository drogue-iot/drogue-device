#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use core::future::Future;
use drogue_device::bsp::boards::nrf52::microbit::*;
use drogue_device::drivers::ble::mesh::bearer::nrf52::{
    Nrf52BleMeshFacilities, SoftdeviceAdvertisingBearer, SoftdeviceGattBearer, SoftdeviceRng,
};
use drogue_device::drivers::ble::mesh::composition::{
    CompanyIdentifier, Composition, ElementDescriptor, ElementsHandler, Features, Location,
    ProductIdentifier, VersionIdentifier,
};
use drogue_device::drivers::ble::mesh::config::ConfigurationModel;
use drogue_device::drivers::ble::mesh::driver::elements::AppElementsContext;
use drogue_device::drivers::ble::mesh::driver::DeviceError;
use drogue_device::drivers::ble::mesh::interface::AdvertisingAndGattNetworkInterfaces;
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
    drivers::ble::mesh::model::firmware::FIRMWARE_UPDATE_SERVER,
    drivers::ble::mesh::model::sensor::{
        PropertyId, SensorConfig, SensorData, SensorDescriptor, SensorMessage, SensorServer,
        SensorStatus, SENSOR_SERVER,
    },
    Board, DeviceContext,
};
use embassy::channel::{Channel, DynamicReceiver, DynamicSender};
use embassy::time::Ticker;
use embassy::time::{Duration, Timer};
use embassy::util::Forever;
use embassy::util::{select, Either};
use embassy::{blocking_mutex::raw::NoopRawMutex, executor::Spawner};
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::Peripherals;
use futures::StreamExt;
use heapless::Vec;
use nrf_softdevice::{temperature_celsius, Softdevice};

use nrf_softdevice::Flash;

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "nrf-softdevice-defmt-rtt")]
use nrf_softdevice_defmt_rtt as _;

#[cfg(feature = "panic-reset")]
use panic_reset as _;

type ConcreteMeshNode = MeshNode<
    'static,
    MicrobitElementsHandler,
    AdvertisingAndGattNetworkInterfaces<SoftdeviceAdvertisingBearer, SoftdeviceGattBearer, 66>,
    FlashStorage<Flash>,
    SoftdeviceRng,
>;

pub struct MyDevice {
    mesh: Forever<ConcreteMeshNode>,
    control: Channel<NoopRawMutex, MeshNodeMessage, 1>,
    publisher: Channel<NoopRawMutex, PublisherMessage, 2>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

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

#[embassy::main(config = "config()")]
async fn main(spawner: Spawner, p: Peripherals) {
    let board = Microbit::new(p);
    let facilities = Nrf52BleMeshFacilities::new("Drogue IoT BLE Mesh");
    let advertising_bearer = facilities.advertising_bearer();
    let gatt_bearer = facilities.gatt_bearer();
    let rng = facilities.rng();
    let storage = FlashStorage::new(
        unsafe { &__storage as *const u8 as usize },
        facilities.flash(),
    );

    let capabilities = Capabilities {
        number_of_elements: 2,
        algorithms: Algorithms::default(),
        public_key_type: PublicKeyType::default(),
        static_oob_type: StaticOOBType::default(),
        output_oob_size: OOBSize::MaximumSize(4),
        output_oob_action: OutputOOBActions::default(),
        input_oob_size: OOBSize::MaximumSize(4),
        input_oob_action: InputOOBActions::default(),
    };

    let device = DEVICE.configure(MyDevice {
        mesh: Forever::new(),
        publisher: Channel::new(),
        control: Channel::new(),
    });

    let mut composition = Composition::new(
        COMPANY_IDENTIFIER,
        PRODUCT_IDENTIFIER,
        VERSION_IDENTIFIER,
        FEATURES,
    );
    composition
        .add_element(
            ElementDescriptor::new(Location(0x0001))
                .add_model(SENSOR_SERVER)
                .add_model(FIRMWARE_UPDATE_SERVER),
        )
        .ok();

    let elements = MicrobitElementsHandler {
        composition,
        display: board.display,
        publisher: device.publisher.sender().into(),
    };

    //let network = AdvertisingOnlyNetworkInterfaces::new(advertising_bearer);
    let network = AdvertisingAndGattNetworkInterfaces::new(advertising_bearer, gatt_bearer);
    let mesh_node = MeshNode::new(elements, capabilities, network, storage, rng);
    let mesh_node = device.mesh.put(mesh_node);

    let version = FIRMWARE_REVISION.unwrap_or(FIRMWARE_VERSION);
    defmt::info!("Running firmware version {}", version);

    spawner.spawn(softdevice_task(facilities.sd())).unwrap();

    spawner
        .spawn(mesh_task(mesh_node, device.control.receiver().into()))
        .unwrap();

    spawner
        .spawn(publisher_task(
            Duration::from_secs(60),
            facilities.sd(),
            device.publisher.receiver().into(),
        ))
        .unwrap();

    spawner.spawn(watchdog_task()).unwrap();
}

#[embassy::task]
async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}

#[embassy::task]
pub async fn mesh_task(
    node: &'static mut ConcreteMeshNode,
    control: DynamicReceiver<'static, MeshNodeMessage>,
) {
    node.run(control).await;
}

#[embassy::task]
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
                let value = value as i16;
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
#[embassy::task]
async fn watchdog_task() {
    let mut handle = unsafe { embassy_nrf::wdt::WatchdogHandle::steal(0) };
    loop {
        handle.pet();
        Timer::after(Duration::from_secs(2)).await;
    }
}

#[allow(unused)]
pub struct MicrobitElementsHandler {
    composition: Composition,
    display: LedMatrix,
    publisher: DynamicSender<'static, PublisherMessage>,
}

pub enum PublisherMessage {
    Connect(AppElementsContext<'static>),
    SetPeriod(Duration),
}

impl ElementsHandler<'static> for MicrobitElementsHandler {
    fn composition(&self) -> &Composition {
        &self.composition
    }

    fn connect(&mut self, ctx: AppElementsContext<'static>) {
        let _ = self
            .publisher
            .try_send(PublisherMessage::Connect(ctx.clone()));
    }

    fn configure(&mut self, config: &ConfigurationModel) {
        if let Some(period) = config.publish_period_duration() {
            let _ = self.publisher.try_send(PublisherMessage::SetPeriod(period));
        }
    }

    type DispatchFuture<'m> = impl Future<Output = Result<(), DeviceError>> + 'm where Self: 'm;
    fn dispatch<'m>(
        &'m mut self,
        _element: u8,
        _model_identifier: &'m ModelIdentifier,
        _message: &'m AccessMessage,
    ) -> Self::DispatchFuture<'m> {
        async move { Ok(()) }
    }
}

#[derive(Clone)]
pub struct SensorModel;

pub struct Temperature(i16);

impl SensorConfig for SensorModel {
    type Data<'m> = Temperature;

    const DESCRIPTORS: &'static [SensorDescriptor] = &[SensorDescriptor::new(PropertyId(1), 1)];
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
