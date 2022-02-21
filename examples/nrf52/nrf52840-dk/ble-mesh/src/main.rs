#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use core::future::Future;
#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;
use drogue_device::actors::ble::mesh::MeshNode;
use drogue_device::drivers::ble::mesh::bearer::nrf52::{
    Nrf52BleMeshFacilities, SoftdeviceAdvertisingBearer, SoftdeviceRng,
};
use drogue_device::drivers::ble::mesh::composition::{
    CompanyIdentifier, Composition, ElementDescriptor, ElementsHandler, Features, Location,
    ProductIdentifier, VersionIdentifier,
};
use drogue_device::drivers::ble::mesh::driver::elements::{AppElementContext, AppElementsContext};
use drogue_device::drivers::ble::mesh::driver::DeviceError;
use drogue_device::drivers::ble::mesh::model::generic::onoff::{GENERIC_ONOFF_CLIENT, GenericOnOffClient, GenericOnOffMessage, Set};
use drogue_device::drivers::ble::mesh::pdu::access::AccessMessage;
use drogue_device::drivers::ble::mesh::provisioning::{
    Algorithms, Capabilities, InputOOBActions, OOBSize, OutputOOBActions, PublicKeyType,
    StaticOOBType,
};
use drogue_device::drivers::ble::mesh::storage::FlashStorage;
use drogue_device::drivers::ActiveHigh;
use drogue_device::{actors, drivers, ActorContext, Address, DeviceContext, Actor, Inbox};
use drogue_device::actors::button::ButtonEventHandler;
use drogue_device::traits::button::Event;
use embassy::executor::Spawner;
use embassy_nrf::config::Config;
use embassy_nrf::gpio::{Level, OutputDrive, Pull};
use embassy_nrf::interrupt::Priority;
use embassy_nrf::peripherals::{
    P0_06,
    P0_11,
};
use embassy_nrf::{gpio::Output, gpio::Input, Peripherals};
use drogue_device::drivers::ActiveLow;

use nrf_softdevice::Flash;

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(not(feature = "panic-probe"))]
use panic_reset as _;

pub struct MyDevice {
    #[allow(dead_code)]
    led: ActorContext<actors::led::Led<drivers::led::Led<Output<'static, P0_06>>>>,
    button_publisher: ActorContext<MeshButtonPublisher>,
    button: ActorContext<actors::button::Button<drivers::button::Button<Input<'static, P0_11>, ActiveLow>, MeshButtonPublisherConnector>>,
    facilities: ActorContext<Nrf52BleMeshFacilities>,
    mesh: ActorContext<
        MeshNode<
            CustomElementsHandler,
            SoftdeviceAdvertisingBearer,
            FlashStorage<Flash>,
            SoftdeviceRng,
        >,
    >,
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

#[embassy::main(config = "config()")]
async fn main(spawner: Spawner, p: Peripherals) {
    let facilities = Nrf52BleMeshFacilities::new("Drogue IoT BLE Mesh");
    let bearer = facilities.bearer();
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

    let device = DEVICE.configure(MyDevice {
        led: ActorContext::new(),
        button: ActorContext::new(),
        button_publisher: ActorContext::new(),
        facilities: ActorContext::new(),
        mesh: ActorContext::new(),
    });

    let led = actors::led::Led::new(drivers::led::Led::<_, ActiveHigh>::new(Output::new(
        p.P0_06,
        Level::High,
        OutputDrive::Standard,
    )));

    let led = device.led.mount(spawner, led);

    let button_publisher = MeshButtonPublisher::new();
    let button_publisher = device.button_publisher.mount(spawner, button_publisher);

    let button_publisher_connector = MeshButtonPublisherConnector(button_publisher);

    let button = actors::button::Button::new(drivers::button::Button::new(Input::new(p.P0_11, Pull::Up)), button_publisher_connector);
    let _button = device.button.mount(spawner, button);

    let mut composition = Composition::new(
        COMPANY_IDENTIFIER,
        PRODUCT_IDENTIFIER,
        VERSION_IDENTIFIER,
        FEATURES,
    );
    composition
        .add_element(ElementDescriptor::new(Location(0x0001)).add_model(GENERIC_ONOFF_CLIENT))
        .ok();

    let elements = CustomElementsHandler { composition, led, button: button_publisher };



    device.facilities.mount(spawner, facilities);
    let mesh_node = MeshNode::new(elements, capabilities, bearer, storage, rng);
    //let mesh_node = MeshNode::new(capabilities, bearer, storage, rng).force_reset();
    device.mesh.mount(spawner, mesh_node);
}

#[allow(unused)]
pub struct CustomElementsHandler {
    composition: Composition,
    led: Address<actors::led::Led<drivers::led::Led<Output<'static, P0_06>>>>,
    button: Address<MeshButtonPublisher>,
}

impl CustomElementsHandler {}

impl ElementsHandler for CustomElementsHandler {
    fn composition(&self) -> &Composition {
        &self.composition
    }

    fn connect(&self, ctx: AppElementsContext) {
        let button_ctx = ctx.for_element_model::<GenericOnOffClient>(0);
        self.button.notify( MeshButtonMessage::Connect(button_ctx)).ok();
        defmt::info!("connecting!");
    }

    type DispatchFuture<'m>
        where
            Self: 'm,
    = impl Future<Output=Result<(), DeviceError>> + 'm;

    fn dispatch(&self, _element: u8, _message: AccessMessage) -> Self::DispatchFuture<'_> {
        async move { todo!() }
    }
}


pub enum MeshButtonMessage {
    Connect(AppElementContext<GenericOnOffClient>),
    Event(Event),
}

pub struct MeshButtonPublisher {
    ctx: Option<AppElementContext<GenericOnOffClient>>,
}

impl MeshButtonPublisher {
    pub fn new() -> Self {
        Self {
            ctx: None
        }
    }
}

impl Default for MeshButtonPublisher {
    fn default() -> Self {
        Self::new()
    }
}

impl Actor for MeshButtonPublisher {
    type Message<'m> = MeshButtonMessage;
    type OnMountFuture<'m, M>
        where Self: 'm,
              M: 'm
    = impl Future<Output=()> + 'm;

    fn on_mount<'m, M>(&'m mut self, _: Address<Self>, inbox: &'m mut M) -> Self::OnMountFuture<'m, M> where M: Inbox<Self> + 'm {
        async move {
            loop {
                if let Some(mut message) = inbox.next().await {
                    match message.message() {
                        MeshButtonMessage::Connect(ctx) => {
                            defmt::info!("connected to mesh {}", ctx.address());
                            self.ctx.replace(ctx.clone());
                        },
                        MeshButtonMessage::Event(event) => {
                            match event {
                                Event::Pressed => {
                                    defmt::info!("pressed");
                                    if let Some(ctx) = &self.ctx {
                                        ctx.publish( GenericOnOffMessage::SetUnacknowledged(
                                            Set {
                                                on_off: 1,
                                                tid: 0,
                                                transition_time: 0,
                                                delay: 0
                                            }
                                        )).await.ok();
                                    }
                                }
                                Event::Released => {
                                    defmt::info!("released");
                                    if let Some(ctx) = &self.ctx {
                                        ctx.publish( GenericOnOffMessage::SetUnacknowledged(
                                            Set {
                                                on_off: 0,
                                                tid: 0,
                                                transition_time: 0,
                                                delay: 0
                                            }
                                        )).await.ok();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub struct MeshButtonPublisherConnector(
    Address<MeshButtonPublisher>
);

impl ButtonEventHandler for MeshButtonPublisherConnector {
    fn handle(&mut self, event: Event) {
        self.0.notify( MeshButtonMessage::Event(event) ).ok();
    }
}