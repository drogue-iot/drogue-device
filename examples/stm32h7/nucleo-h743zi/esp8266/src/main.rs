#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_temperature::*;

use drogue_device::{
    actors::{
        button::Button,
        wifi::{esp8266::*, AdapterActor},
    },
    drivers::wifi::esp8266::Esp8266Controller,
    traits::wifi::*,
    ActorContext, DeviceContext, Package,
};
use embassy::util::Forever;
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::interrupt;
use embassy_stm32::usart::{BufferedUart, Config, State, Uart};
use embassy_stm32::{dma::NoDma, peripherals::UART7};
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    peripherals::{PC13, PD12, PD13},
    Peripherals,
};

cfg_if::cfg_if! {
    if #[cfg(feature = "tls")] {
        use drogue_tls::{Aes128GcmSha256};
        use drogue_device::actors::net::TlsConnectionFactory;
        use embassy_stm32::{rng::Rng, peripherals::RNG};

        const HOST: &str = "http.sandbox.drogue.cloud";
        const PORT: u16 = 443;
        static mut TLS_BUFFER: [u8; 16384] = [0; 16384];
    } else {
        use drogue_device::Address;

        const HOST: &str = "localhost";
        const PORT: u16 = 8088;
    }
}

const WIFI_SSID: &str = include_str!(concat!(env!("OUT_DIR"), "/", "wifi-ssid"));
const WIFI_PSK: &str = include_str!(concat!(env!("OUT_DIR"), "/", "wifi-password"));
const USERNAME: &str = include_str!(concat!(env!("OUT_DIR"), "/", "http-username"));
const PASSWORD: &str = include_str!(concat!(env!("OUT_DIR"), "/", "http-password"));

type UART = BufferedUart<'static, UART7>;
type ENABLE = Output<'static, PD13>;
type RESET = Output<'static, PD12>;

#[cfg(feature = "tls")]
type ConnectionFactory = TlsConnectionFactory<
    'static,
    AdapterActor<Esp8266Controller<'static>>,
    Aes128GcmSha256,
    Rng<RNG>,
    1,
>;

#[cfg(not(feature = "tls"))]
type ConnectionFactory = Address<'static, AdapterActor<Esp8266Controller<'static>>>;

pub struct MyDevice {
    wifi: Esp8266Wifi<UART, ENABLE, RESET>,
    app: ActorContext<'static, App<ConnectionFactory>>,
    button:
        ActorContext<'static, Button<'static, ExtiInput<'static, PC13>, App<ConnectionFactory>>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
    }

    let button = Input::new(p.PC13, Pull::Down);
    let button = ExtiInput::new(button, p.EXTI13);

    let enable_pin = Output::new(p.PD13, Level::Low, Speed::Low);
    let reset_pin = Output::new(p.PD12, Level::Low, Speed::Low);

    static mut TX_BUFFER: [u8; 1] = [0; 1];
    static mut RX_BUFFER: [u8; 1024] = [0; 1024];
    static STATE: Forever<State<'static, UART7>> = Forever::new();

    let usart = Uart::new(p.UART7, p.PF6, p.PF7, NoDma, NoDma, Config::default());
    let usart = unsafe {
        let state = STATE.put(State::new());
        BufferedUart::new(
            state,
            usart,
            interrupt::take!(UART7),
            &mut TX_BUFFER,
            &mut RX_BUFFER,
        )
    };

    #[cfg(feature = "tls")]
    let rng = Rng::new(p.RNG);

    DEVICE.configure(MyDevice {
        wifi: Esp8266Wifi::new(usart, enable_pin, reset_pin),
        app: ActorContext::new(App::new(
            HOST,
            PORT,
            USERNAME.trim_end(),
            PASSWORD.trim_end(),
        )),
        button: ActorContext::new(Button::new(button)),
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
            let factory = TlsConnectionFactory::new(factory, rng, [unsafe { &mut TLS_BUFFER }; 1]);

            let app = device.app.mount(factory, spawner);
            device.button.mount(app, spawner);
            app.request(Command::Update(TemperatureData {
                temp: Some(22.0),
                hum: None,
                geoloc: None,
            }))
            .unwrap()
            .await;
            app
        })
        .await;
    defmt::info!("Application initialized. Press 'A' button to send data");
}
