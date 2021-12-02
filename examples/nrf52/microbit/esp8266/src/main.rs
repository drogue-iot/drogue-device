#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_temperature::*;

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    actors::{
        button::{Button, ButtonEventDispatcher},
        wifi::{esp8266::*, AdapterActor},
    },
    drivers::wifi::esp8266::Esp8266Controller,
    drogue,
    traits::wifi::*,
    ActorContext, DeviceContext, Package,
};
use embassy::{time::Duration, util::Forever};
use embassy_nrf::{
    buffered_uarte::{BufferedUarte, State},
    gpio::{Input, Level, NoPin, Output, OutputDrive, Pull},
    gpiote::PortInput,
    interrupt,
    peripherals::{P0_09, P0_10, P0_14, TIMER0, UARTE0},
    temp::Temp,
    uarte, Peripherals,
};

mod temperature;
use temperature::TemperatureMonitor;

cfg_if::cfg_if! {
    if #[cfg(feature = "tls")] {
        mod rng;
        use rng::*;
        use drogue_tls::{Aes128GcmSha256};
        use drogue_device::actors::net::TlsConnectionFactory;
        use nrf52833_pac as pac;

        const HOST: &str = "http.sandbox.drogue.cloud";
        const PORT: u16 = 443;
        static mut TLS_BUFFER: [u8; 16384] = [0; 16384];
    } else {
        use drogue_device::Address;

        const HOST: &str = "localhost";
        const PORT: u16 = 8088;
    }
}

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");
const USERNAME: &str = drogue::config!("http-username");
const PASSWORD: &str = drogue::config!("http-password");

type UART = BufferedUarte<'static, UARTE0, TIMER0>;
type ENABLE = Output<'static, P0_09>;
type RESET = Output<'static, P0_10>;

#[cfg(feature = "tls")]
type ConnectionFactory = TlsConnectionFactory<
    'static,
    AdapterActor<Esp8266Controller<'static>>,
    Aes128GcmSha256,
    Rng,
    1,
>;

#[cfg(not(feature = "tls"))]
type ConnectionFactory = Address<'static, AdapterActor<Esp8266Controller<'static>>>;

pub struct MyDevice {
    wifi: Esp8266Wifi<UART, ENABLE, RESET>,
    app: ActorContext<'static, App<ConnectionFactory>>,
    button: ActorContext<
        'static,
        Button<PortInput<'static, P0_14>, ButtonEventDispatcher<App<ConnectionFactory>>>,
    >,
    temperature: ActorContext<'static, TemperatureMonitor<'static>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let button_port = PortInput::new(Input::new(p.P0_14, Pull::Up));

    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    static mut TX_BUFFER: [u8; 8192] = [0u8; 8192];
    static mut RX_BUFFER: [u8; 8192] = [0u8; 8192];
    static mut STATE: Forever<State<'static, UARTE0, TIMER0>> = Forever::new();

    let irq = interrupt::take!(UARTE0_UART0);
    let u = unsafe {
        let state = STATE.put(State::new());
        BufferedUarte::new(
            state,
            p.UARTE0,
            p.TIMER0,
            p.PPI_CH0,
            p.PPI_CH1,
            irq,
            p.P0_13,
            p.P0_01,
            NoPin,
            NoPin,
            config,
            &mut RX_BUFFER,
            &mut TX_BUFFER,
        )
    };

    let enable_pin = Output::new(p.P0_09, Level::Low, OutputDrive::Standard);
    let reset_pin = Output::new(p.P0_10, Level::Low, OutputDrive::Standard);

    DEVICE.configure(MyDevice {
        wifi: Esp8266Wifi::new(u, enable_pin, reset_pin),
        app: ActorContext::new(App::new(
            HOST,
            PORT,
            USERNAME.trim_end(),
            PASSWORD.trim_end(),
        )),
        button: ActorContext::new(Button::new(button_port)),
        temperature: ActorContext::new(TemperatureMonitor::new(
            Temp::new(p.TEMP, interrupt::take!(TEMP)),
            Duration::from_secs(10),
        )),
    });

    DEVICE
        .mount(|device| async move {
            let mut wifi = device.wifi.mount((), spawner);
            wifi.join(Join::Wpa {
                ssid: WIFI_SSID.trim_end(),
                password: WIFI_PSK.trim_end(),
            })
            .await
            .expect("Error joining wifi");
            defmt::info!("WiFi network joined");

            let factory = wifi;
            #[cfg(feature = "tls")]
            let factory = TlsConnectionFactory::new(
                factory,
                Rng::new(pac::Peripherals::take().unwrap().RNG),
                [unsafe { &mut TLS_BUFFER }; 1],
            );

            let app = device.app.mount(factory, spawner);
            device.button.mount(app.into(), spawner);
            device.temperature.mount(app, spawner);
        })
        .await;
    defmt::info!("Application initialized. Press 'A' button to send data");
}
