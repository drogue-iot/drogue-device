#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

mod rng;
use rng::*;

use drogue_device::{
    bsp::{boards::nrf52::microbit::*, Board},
    domain::temperature::Celsius,
    drivers::wifi::esp8266::*,
    network::tcp::*,
    *,
};
use drogue_device::{drogue, traits::wifi::*, DeviceContext};
use drogue_temperature::*;
use embassy::util::Forever;
use embassy_nrf::{
    buffered_uarte::{BufferedUarte, State},
    gpio::{Level, Output, OutputDrive},
    interrupt,
    peripherals::{P0_09, P0_10, TIMER0, UARTE0},
    uarte, Peripherals,
};

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

type SERIAL = BufferedUarte<'static, UARTE0, TIMER0>;
type ENABLE = Output<'static, P0_09>;
type RESET = Output<'static, P0_10>;

bind_bsp!(Microbit, BSP);

impl TemperatureBoard for BSP {
    type Network = SharedTcpStack<'static, Esp8266Modem<SERIAL, ENABLE, RESET>>;
    type TemperatureScale = Celsius;
    type SensorReadyIndicator = AlwaysReady;
    type Sensor = TemperatureMonitor;
    type SendTrigger = PinButtonA;
    type Rng = Rng;
}

static DEVICE: DeviceContext<TemperatureDevice<BSP>> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = Microbit::new(p);

    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    static mut TX_BUFFER: [u8; 4096] = [0u8; 4096];
    static mut RX_BUFFER: [u8; 4096] = [0u8; 4096];
    let irq = interrupt::take!(UARTE0_UART0);
    static STATE: Forever<State<'static, UARTE0, TIMER0>> = Forever::new();
    let state = STATE.put(State::new());
    let uart = BufferedUarte::new(
        state,
        board.uarte0,
        board.timer0,
        board.ppi_ch0,
        board.ppi_ch1,
        irq,
        board.p15,
        board.p14,
        board.p1,
        board.p2,
        config,
        unsafe { &mut RX_BUFFER },
        unsafe { &mut TX_BUFFER },
    );

    let enable_pin = Output::new(board.p9, Level::Low, OutputDrive::Standard);
    let reset_pin = Output::new(board.p8, Level::Low, OutputDrive::Standard);

    let mut network = Esp8266Modem::new(uart, enable_pin, reset_pin);
    network.initialize().await.unwrap();

    network
        .join(Join::Wpa {
            ssid: WIFI_SSID.trim_end(),
            password: WIFI_PSK.trim_end(),
        })
        .await
        .expect("Error joining WiFi network");

    static NETWORK: TcpStackState<Esp8266Modem<SERIAL, ENABLE, RESET>> = TcpStackState::new();
    let network = NETWORK.initialize(network);

    let config = TemperatureBoardConfig {
        send_trigger: board.btn_a,
        sensor: board.temp,
        sensor_ready: AlwaysReady,
        network,
    };

    #[cfg(feature = "tls")]
    defmt::info!("Application configured to use TLS");

    #[cfg(not(feature = "tls"))]
    defmt::info!("Application configured to NOT use TLS");

    DEVICE
        .configure(TemperatureDevice::new())
        .mount(
            spawner,
            Rng::new(nrf52833_pac::Peripherals::take().unwrap().RNG),
            config,
        )
        .await;
    defmt::info!("Application initialized. Press 'A' button to send data");
}
