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
use drogue_device::drivers::ble::mesh::provisioning::{
    Algorithms, Capabilities, InputOOBActions, OOBSize, OutputOOBActions, PublicKeyType,
    StaticOOBType,
};
use drogue_device::{actors, drivers, ActorContext, DeviceContext};
use embassy::executor::Spawner;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::{
    gpio::{AnyPin, Output},
    Peripherals,
};
use panic_probe as _;

pub struct MyDevice {
    #[allow(dead_code)]
    led: ActorContext<actors::led::Led<drivers::led::Led<Output<'static, AnyPin>>>>,
    facilities: ActorContext<Nrf52BleMeshFacilities>,
    mesh: ActorContext<MeshNode<SoftdeviceAdvertisingBearer, SoftdeviceStorage, SoftdeviceRng>>,
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

#[embassy::main(config = "config()")]
async fn main(spawner: Spawner, _p: Peripherals) {
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
    device.facilities.mount(spawner, facilities);
    let mesh_node = MeshNode::new(capabilities, bearer, storage, rng);
    //let mesh_node = MeshNode::new(capabilities, bearer, storage, rng).force_reset();
    device.mesh.mount(spawner, mesh_node);
}
