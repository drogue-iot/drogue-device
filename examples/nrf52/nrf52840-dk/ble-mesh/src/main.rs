#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use core::{
    convert::{Infallible, TryFrom},
    future::Future,
};
#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;
use drogue_device::actors::ble::mesh::{MeshNode, MeshNodeMessage, NodeMutex};
use drogue_device::actors::button::ButtonEvent;
use drogue_device::actors::led::LedMessage;
use drogue_device::drivers::ble::mesh::bearer::nrf52::{
    Nrf52BleMeshFacilities, SoftdeviceAdvertisingBearer, SoftdeviceGattBearer, SoftdeviceRng,
};
use drogue_device::drivers::ble::mesh::composition::{
    CompanyIdentifier, Composition, ElementDescriptor, ElementsHandler, Features, Location,
    ProductIdentifier, VersionIdentifier,
};
use drogue_device::drivers::ble::mesh::driver::elements::{AppElementContext, AppElementsContext};
use drogue_device::drivers::ble::mesh::driver::DeviceError;
use drogue_device::drivers::ble::mesh::interface::AdvertisingAndGattNetworkInterfaces;
use drogue_device::drivers::ble::mesh::model::generic::onoff::{
    GenericOnOffClient, GenericOnOffMessage, GenericOnOffServer, Set, GENERIC_ONOFF_CLIENT,
    GENERIC_ONOFF_SERVER,
};
use drogue_device::drivers::ble::mesh::model::{Model, ModelIdentifier};
use drogue_device::drivers::ble::mesh::pdu::access::AccessMessage;
use drogue_device::drivers::ble::mesh::provisioning::{
    Algorithms, Capabilities, InputOOBActions, OOBSize, OutputOOBActions, PublicKeyType,
    StaticOOBType,
};
use drogue_device::drivers::ble::mesh::storage::FlashStorage;
use drogue_device::drivers::ActiveLow;
use drogue_device::traits::button::Event;
use drogue_device::{actors, drivers};
use ector::{Actor, ActorContext, Address, Inbox};
use embassy_util::channel::mpmc::{Channel, DynamicReceiver, Sender};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embassy_util::Forever;
use embassy_nrf::config::Config;
use embassy_nrf::gpio::{Level, OutputDrive, Pull};
use embassy_nrf::interrupt::Priority;
use embassy_nrf::peripherals::{P0_11, P0_13, P0_25};
use embassy_nrf::{gpio::Input, gpio::Output, Peripherals};
use futures::future::{select, Either};
use futures::pin_mut;

use nrf_softdevice::Flash;

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(not(feature = "panic-probe"))]
use panic_reset as _;

type ConcreteMeshNode = MeshNode<
    'static,
    CustomElementsHandler,
    AdvertisingAndGattNetworkInterfaces<SoftdeviceAdvertisingBearer, SoftdeviceGattBearer, 66>,
    FlashStorage<Flash>,
    SoftdeviceRng,
>;

pub struct MyDevice {
    #[allow(dead_code)]
    led: ActorContext<actors::led::Led<drivers::led::Led<Output<'static, P0_13>, ActiveLow>>>,
    button_publisher: ActorContext<MeshButtonPublisher>,
    button: ActorContext<
        actors::button::Button<
            drivers::button::Button<Input<'static, P0_11>, ActiveLow>,
            MeshButtonMessage,
        >,
    >,
    facilities: ActorContext<Nrf52BleMeshFacilities>,
    mesh: Forever<ConcreteMeshNode>,
    reset: ActorContext<MeshNodeReset>,
    reset_button: ActorContext<
        actors::button::Button<
            drivers::button::Button<Input<'static, P0_25>, ActiveLow>,
            ButtonEvent,
        >,
    >,
}

static DEVICE: Forever<MyDevice> = Forever::new();

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

#[embassy_executor::main(config = "config()")]
async fn main(spawner: Spawner, p: Peripherals) {
    let facilities = Nrf52BleMeshFacilities::new("Drogue IoT BLE Mesh", true);
    let advertising_bearer = facilities.advertising_bearer();
    let gatt_bearer = facilities.gatt_bearer();
    let rng = facilities.rng();
    let storage = FlashStorage::new(
        unsafe { &__storage as *const u8 as usize },
        facilities.flash(),
    );

    let capabilities = Capabilities {
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
        led: ActorContext::new(),
        button: ActorContext::new(),
        button_publisher: ActorContext::new(),
        facilities: ActorContext::new(),
        mesh: Forever::new(),
        reset: ActorContext::new(),
        reset_button: ActorContext::new(),
    });

    let led = actors::led::Led::new(drivers::led::Led::<_, ActiveLow>::new(Output::new(
        p.P0_13,
        Level::Low,
        OutputDrive::Standard,
    )));

    let led = device.led.mount(spawner, led);

    let button_publisher = MeshButtonPublisher::new();
    let button_publisher = device.button_publisher.mount(spawner, button_publisher);

    let button = actors::button::Button::new(
        drivers::button::Button::new(Input::new(p.P0_11, Pull::Up)),
        button_publisher.clone(),
    );
    let _button = device.button.mount(spawner, button);

    let mut composition = Composition::new(
        COMPANY_IDENTIFIER,
        PRODUCT_IDENTIFIER,
        VERSION_IDENTIFIER,
        FEATURES,
    );
    composition
        .add_element(
            ElementDescriptor::new(Location(0x0001))
                .add_model(GENERIC_ONOFF_CLIENT) /* the button */
                .add_model(GENERIC_ONOFF_SERVER), /* the LED */
        )
        .ok();

    let elements = CustomElementsHandler {
        composition,
        led,
        button: button_publisher,
    };

    device.facilities.mount(spawner, facilities);
    //let network = AdvertisingOnlyNetworkInterfaces::new(advertising_bearer);
    let network = AdvertisingAndGattNetworkInterfaces::new(advertising_bearer, gatt_bearer);

    let mesh_node = MeshNode::new(elements, capabilities, network, storage, rng);
    let mesh_node = device.mesh.put(mesh_node);

    static CONTROL: Channel<NodeMutex, MeshNodeMessage, 2> = Channel::new();
    spawner
        .spawn(run(mesh_node, CONTROL.receiver().into()))
        .unwrap();

    let reset = MeshNodeReset(CONTROL.sender());
    let reset = device.reset.mount(spawner, reset);

    let reset_button = actors::button::Button::new(
        drivers::button::Button::new(Input::new(p.P0_25, Pull::Up)),
        reset,
    );
    let _reset_button = device.reset_button.mount(spawner, reset_button);
}

#[embassy_executor::task]
pub async fn run(
    node: &'static mut ConcreteMeshNode,
    control: DynamicReceiver<'static, MeshNodeMessage>,
) {
    node.run(control).await;
}

#[allow(unused)]
pub struct CustomElementsHandler {
    composition: Composition,
    led: Address<LedMessage>,
    button: Address<MeshButtonMessage>,
}

impl ElementsHandler<'static> for CustomElementsHandler {
    fn composition(&self) -> &Composition {
        &self.composition
    }

    fn connect(&mut self, ctx: AppElementsContext<'static>) {
        let button_ctx = ctx.for_element_model::<GenericOnOffClient>(0);
        self.button
            .try_notify(MeshButtonMessage::Connect(button_ctx))
            .ok();
    }

    type DispatchFuture<'m> = impl Future<Output = Result<(), DeviceError>> + 'm where Self: 'm;

    fn dispatch<'m>(
        &'m mut self,
        element: u8,
        model_identifier: &'m ModelIdentifier,
        message: &'m AccessMessage,
    ) -> Self::DispatchFuture<'m> {
        async move {
            if element == 0 && *model_identifier == GENERIC_ONOFF_SERVER {
                if let Ok(Some(message)) =
                    GenericOnOffServer::parse(message.opcode(), message.parameters())
                {
                    match message {
                        GenericOnOffMessage::Set(set) => {
                            if set.on_off == 0 {
                                self.led.notify(LedMessage::Off).await;
                            } else {
                                self.led.notify(LedMessage::On).await;
                            }
                        }
                        _ => {
                            defmt::warn!("unhandled {}", message);
                        }
                    }
                }
            }
            Ok(())
        }
    }
}

pub enum MeshButtonMessage {
    Connect(AppElementContext<'static, GenericOnOffClient>),
    Event(Event),
}

pub struct MeshButtonPublisher {
    ctx: Option<AppElementContext<'static, GenericOnOffClient>>,
}

impl MeshButtonPublisher {
    pub fn new() -> Self {
        Self { ctx: None }
    }
}

impl Default for MeshButtonPublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl Actor for MeshButtonPublisher {
    type Message<'m> = MeshButtonMessage;
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<MeshButtonMessage>;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<MeshButtonMessage>,
        mut inbox: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<MeshButtonMessage> + 'm,
    {
        async move {
            loop {
                match inbox.next().await {
                    MeshButtonMessage::Connect(ctx) => {
                        defmt::info!("connected to mesh {}", ctx.address());
                        self.ctx.replace(ctx.clone());
                    }
                    MeshButtonMessage::Event(event) => match event {
                        Event::Pressed => {
                            if let Some(ctx) = &self.ctx {
                                ctx.publish(GenericOnOffMessage::SetUnacknowledged(Set {
                                    on_off: 1,
                                    tid: 0,
                                    transition_time: None,
                                    delay: None,
                                }))
                                .await
                                .ok();
                            }
                        }
                        Event::Released => {
                            if let Some(ctx) = &self.ctx {
                                ctx.publish(GenericOnOffMessage::SetUnacknowledged(Set {
                                    on_off: 0,
                                    tid: 0,
                                    transition_time: None,
                                    delay: None,
                                }))
                                .await
                                .ok();
                            }
                        }
                    },
                }
            }
        }
    }
}

pub struct MeshButtonPublisherConnector(Address<MeshButtonPublisher>);

impl TryFrom<ButtonEvent> for MeshButtonMessage {
    type Error = Infallible;
    fn try_from(event: ButtonEvent) -> Result<Self, Self::Error> {
        Ok(MeshButtonMessage::Event(event))
    }
}

pub struct ResetButtonHandler(Address<MeshNodeMessage>);
pub struct MeshNodeReset(Sender<'static, NodeMutex, MeshNodeMessage, 2>);

impl Actor for MeshNodeReset {
    type Message<'m> = ButtonEvent;
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        M: 'm + Inbox<ButtonEvent>;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<ButtonEvent>,
        mut inbox: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<ButtonEvent> + 'm,
        Self: 'm,
    {
        async move {
            loop {
                match inbox.next().await {
                    ButtonEvent::Pressed => {
                        defmt::warn!("continue holding button 4 for 5 seconds to perform reset");
                        let next_event_fut = inbox.next();
                        let timeout_fut = Timer::after(Duration::from_secs(5));

                        pin_mut!(next_event_fut);
                        pin_mut!(timeout_fut);

                        let result = select(next_event_fut, timeout_fut).await;
                        match result {
                            Either::Left((_, _)) => {
                                // nothing
                                defmt::warn!("reset cancelled")
                            }
                            Either::Right((_, _)) => {
                                defmt::warn!("performing reset");
                                self.0.send(MeshNodeMessage::ForceReset).await;
                            }
                        }
                    }
                    ButtonEvent::Released => {
                        // nothing
                    }
                }
            }
        }
    }
}
