#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    bind_bsp,
    bsp::{boards::stm32h7::nucleo_h743zi::*, Board},
    domain::temperature::Celsius,
};
use drogue_temperature::*;
use embassy_net::{
    tcp::client::{TcpClient, TcpClientState},
    Stack, StackResources,
};
use embassy_stm32::{peripherals::RNG, rng::Rng};
use rand_core::RngCore;
use static_cell::StaticCell;

// Creates a newtype named `BSP` around the `NucleoH743` to avoid
// orphan rules and apply delegation boilerplate.
bind_bsp!(NucleoH743, BSP);

impl TemperatureBoard for BSP {
    type Network = TcpClient<'static, EthernetDevice, 1, 1024, 1024>;
    type TemperatureScale = Celsius;
    type SensorReadyIndicator = AlwaysReady;
    type Sensor = FakeSensor;
    type SendTrigger = UserButton;
    type Rng = Rng<'static, RNG>;
}

static DEVICE: StaticCell<TemperatureDevice<BSP>> = StaticCell::new();
static RESOURCES: StaticCell<StackResources<1, 2, 8>> = StaticCell::new();
static STACK: StaticCell<Stack<EthernetDevice>> = StaticCell::new();

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<EthernetDevice>) -> ! {
    stack.run().await
}

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let board = NucleoH743::new(embassy_stm32::init(Default::default()));

    // Generate random seed.
    let mut rng = board.rng;
    let mut seed = [0; 8];
    rng.fill_bytes(&mut seed);
    let seed = u64::from_le_bytes(seed);

    let config = embassy_net::ConfigStrategy::Dhcp;

    let resources = RESOURCES.init(StackResources::new());

    let stack = STACK.init(Stack::new(board.eth, config, resources, seed));
    spawner.spawn(net_task(stack)).unwrap();

    static mut STATE: TcpClientState<1, 1024, 1024> = TcpClientState::new();
    let network = TcpClient::new(stack, unsafe { &mut STATE });

    DEVICE
        .init(TemperatureDevice::new())
        .mount(
            spawner,
            rng,
            TemperatureBoardConfig {
                send_trigger: board.user_button,
                sensor: FakeSensor(22.0),
                sensor_ready: AlwaysReady,
                network,
            },
        )
        .await;
    defmt::info!("Application initialized. Press the blue button to send data");
}
