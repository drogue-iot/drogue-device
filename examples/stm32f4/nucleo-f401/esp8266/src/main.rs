#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]

use defmt_rtt as _;
use panic_probe as _;

use wifi_app::*;

use drogue_device::{
    actors::{
        button::Button,
        socket::Socket,
        wifi::{esp8266::*, AdapterActor},
    },
    domain::{temperature::Temperature, SensorAcquisition},
    drivers::wifi::esp8266::Esp8266Controller,
    traits::{ip::*, wifi::*},
    ActorContext, DeviceContext, Package,
};
use embassy::util::Forever;
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::interrupt;
use embassy_stm32::usart::{BufferedUart, Config, State, Uart};
use embassy_stm32::{dma::NoDma, peripherals::USART6};
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    peripherals::{PC10, PC12, PC13},
    Peripherals,
};

const IP: IpAddress = IpAddress::new_v4(192, 168, 1, 2); // IP for local network server
const PORT: u16 = 12345;

const WIFI_SSID: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.ssid.txt"));
const WIFI_PSK: &str = include_str!(concat!(env!("OUT_DIR"), "/config/wifi.password.txt"));
const USERNAME: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.username.txt"));
const PASSWORD: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.password.txt"));

type UART = BufferedUart<'static, USART6>;
type ENABLE = Output<'static, PC10>;
type RESET = Output<'static, PC12>;

type AppSocket = Socket<'static, AdapterActor<Esp8266Controller<'static>>>;

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

    let enable_pin = Output::new(p.PC10, Level::Low, Speed::Low);
    let reset_pin = Output::new(p.PC12, Level::Low, Speed::Low);

    static mut TX_BUFFER: [u8; 1] = [0; 1];
    static mut RX_BUFFER: [u8; 1024] = [0; 1024];
    static STATE: Forever<State<'static, USART6>> = Forever::new();

    let usart = Uart::new(p.USART6, p.PA12, p.PA11, NoDma, NoDma, Config::default());
    let usart = unsafe {
        let state = STATE.put(State::new());
        BufferedUart::new(
            state,
            usart,
            interrupt::take!(USART6),
            &mut TX_BUFFER,
            &mut RX_BUFFER,
        )
    };

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

            let socket = Socket::new(wifi, wifi.open().await.unwrap());
            let app = device.app.mount(socket, spawner);
            device.button.mount(app, spawner);
            app.request(Command::Update(SensorAcquisition {
                temperature: Temperature::new(22.0),
                relative_humidity: 0.0,
            }))
            .unwrap()
            .await;
            app
        })
        .await;
    defmt::info!("Application initialized. Press button to send data");
}
