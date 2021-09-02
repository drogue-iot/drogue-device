#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]

use defmt_rtt as _;
use panic_probe as _;

use wifi_app::*;

use drogue_device::{
    actors::{button::Button, socket::Socket, wifi::esp8266::*},
    drivers::wifi::esp8266::Esp8266Controller,
    traits::{ip::*, tcp::TcpStack, wifi::*},
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
    peripherals::{PC13, PD12, PD13, RNG},
    rng::Rng,
    Peripherals,
};

cfg_if::cfg_if! {
    if #[cfg(feature = "tls")] {
        use drogue_tls::{Aes128GcmSha256, TlsContext};
        use drogue_device::actors::socket::TlsSocket;

        const HOST: &str = "http.sandbox.drogue.cloud";
        const IP: IpAddress = IpAddress::new_v4(95, 216, 224, 167); // IP resolved for "http.sandbox.drogue.cloud"
        const PORT: u16 = 443;
        static mut TLS_BUFFER: [u8; 16384] = [0u8; 16384];
    } else {
        const IP: IpAddress = IpAddress::new_v4(192, 168, 1, 2); // IP for local network server
        const PORT: u16 = 12345;
    }
}

const WIFI_SSID: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.ssid.txt"));
const WIFI_PSK: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.password.txt"));
const USERNAME: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.username.txt"));
const PASSWORD: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.password.txt"));

type UART = BufferedUart<'static, UART7>;
type ENABLE = Output<'static, PD13>;
type RESET = Output<'static, PD12>;

#[cfg(feature = "tls")]
type AppSocket =
    TlsSocket<'static, Socket<'static, Esp8266Controller<'static>>, Rng<RNG>, Aes128GcmSha256>;

#[cfg(not(feature = "tls"))]
type AppSocket = Socket<'static, Esp8266Controller<'static>>;

pub struct MyDevice {
    wifi: Esp8266Wifi<UART, ENABLE, RESET>,
    app: ActorContext<'static, App<AppSocket>>,
    button: ActorContext<'static, Button<'static, ExtiInput<'static, PC13>, App<AppSocket>>>,
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
    let rng = Rng::new(p.RNG);

    DEVICE.configure(MyDevice {
        wifi: Esp8266Wifi::new(usart, enable_pin, reset_pin),
        app: ActorContext::new(App::new(IP, PORT, USERNAME.trim_end(), PASSWORD.trim_end())),
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

            let socket = Socket::new(wifi, wifi.open().await);
            #[cfg(feature = "tls")]
            let socket = TlsSocket::wrap(
                socket,
                TlsContext::new(rng, unsafe { &mut TLS_BUFFER }).with_server_name(HOST.trim_end()),
            );
            let app = device.app.mount(socket, spawner);
            device.button.mount(app, spawner);
            app
        })
        .await;
    defmt::info!("Application initialized. Press 'A' button to send data");
}
