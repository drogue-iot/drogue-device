#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;
use drogue_device::actors::ble::mesh::MeshNode;
use drogue_device::drivers::ble::mesh::bearer::nrf52::{
    Nrf52BleMeshFacilities, SoftdeviceAdvertisingBearer, SoftdeviceRng, SoftdeviceStorage,
};
use drogue_device::drivers::ActiveHigh;
use drogue_device::drivers::ble::mesh::provisioning::{
    Algorithms, Capabilities, InputOOBActions, OOBSize, OutputOOBActions, PublicKeyType,
    StaticOOBType,
};
use drogue_device::{actors, drivers, ActorContext, DeviceContext, Address};
use drogue_device::drivers::ble::mesh::driver::DeviceError;
use drogue_device::drivers::ble::mesh::driver::elements::ElementContext;
use drogue_device::drivers::ble::mesh::composition::{ElementsHandler, CompanyIdentifier, Composition, ElementDescriptor, Features, ProductIdentifier, VersionIdentifier, Location};
use drogue_device::drivers::ble::mesh::pdu::access::AccessMessage;
use drogue_device::drivers::ble::mesh::model::generic::GENERIC_ON_OFF_MODEL;
use embassy::executor::Spawner;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::{
    gpio::{AnyPin, Output},
    Peripherals,
};
use embassy_nrf::gpio::{Level, OutputDrive};
use embassy_nrf::peripherals::P0_13;
use panic_probe as _;
use core::future::Future;

pub struct MyDevice {
    #[allow(dead_code)]
    led: ActorContext<actors::led::Led<drivers::led::Led<Output<'static, P0_13>>>>,
    facilities: ActorContext<Nrf52BleMeshFacilities>,
    mesh: ActorContext<MeshNode<CustomElementsHandler, SoftdeviceAdvertisingBearer, SoftdeviceStorage, SoftdeviceRng>>,
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
    low_power: false
};

#[embassy::main(config = "config()")]
async fn main(spawner: Spawner, p: Peripherals) {
    let facilities = Nrf52BleMeshFacilities::new("Drogue IoT BLE Mesh");
    let bearer = facilities.bearer();
    let rng = facilities.rng();
    let storage = facilities.storage(unsafe { &__storage as *const u8 as usize });

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
        facilities: ActorContext::new(),
        mesh: ActorContext::new(),
    });

    let led = actors::led::Led::new(
        drivers::led::Led::<_, ActiveHigh>::new(
            Output::new(p.P0_13, Level::High, OutputDrive::Standard)
        )
    );

    let led = device.led.mount(spawner, led);

    let mut composition = Composition::new(COMPANY_IDENTIFIER, PRODUCT_IDENTIFIER, VERSION_IDENTIFIER, FEATURES);
    composition.add_element(
        ElementDescriptor::new(Location(0x0001)).add_model(GENERIC_ON_OFF_MODEL)
    );

    let elements = CustomElementsHandler {
        composition,
        led,
    };

    device.facilities.mount(spawner, facilities);
    let mesh_node = MeshNode::new(elements, capabilities, bearer, storage, rng);
    //let mesh_node = MeshNode::new(capabilities, bearer, storage, rng).force_reset();
    device.mesh.mount(spawner, mesh_node);
}



pub struct CustomElementsHandler {
    composition: Composition,
    led: Address<actors::led::Led<drivers::led::Led<Output<'static, P0_13>>>>,
}

impl CustomElementsHandler {

}

impl ElementsHandler for CustomElementsHandler {
    fn composition(&self) -> &Composition {
        &self.composition
    }

    fn connect<C: ElementContext>(&self, ctx: &C) {
        todo!()
    }

    type DispatchFuture<'m>
        where Self: 'm = impl Future<Output=Result<(), DeviceError>> + 'm;


    fn dispatch<'m>(&'m self, element: u8, message: AccessMessage) -> Self::DispatchFuture<'m> {
        async move {
            todo!()
        }
    }
}
