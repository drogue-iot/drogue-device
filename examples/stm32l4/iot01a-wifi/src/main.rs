#![no_std]
#![no_main]
#![macro_use]
#![allow(incomplete_features)]
#![allow(dead_code)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::drivers::wifi::eswifi::EsWifiController;
use drogue_device::{
    actors::button::*, actors::i2c::*, actors::sensors::hts221::*, actors::wifi::*,
    traits::sensors::temperature::*, traits::wifi::*, *,
};
use drogue_temperature::*;
use embassy::time::{Duration, Timer};
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::rcc::{AHBPrescaler, ClockSrc, PLLClkDiv, PLLMul, PLLSource, PLLSrcDiv};
use embassy_stm32::spi::{self, Config as SpiConfig, Spi};
use embassy_stm32::time::Hertz;
use embassy_stm32::{
    dma::NoDma,
    exti::*,
    gpio::{Input, Level, Output, Pull, Speed},
    i2c, interrupt,
    peripherals::{
        DMA1_CH4,
        DMA1_CH5,
        //DMA2_CH1,
        //DMA2_CH2,
        I2C2,
        PB13,
        PC13,
        PD15,
        PE0,
        PE1,
        PE8,
        SPI3,
    },
    Config, Peripherals,
};

cfg_if::cfg_if! {
    if #[cfg(feature = "tls")] {
        use embassy_stm32::{
            rng::Rng,
            peripherals::RNG,
        };
        use drogue_tls::{Aes128GcmSha256};
        use drogue_device::actors::net::TlsConnectionFactory;

        const HOST: &str = "http.sandbox.drogue.cloud";
        const PORT: u16 = 443;
        //const HOST: &str = "192.168.1.2";
        //const PORT: u16 = 8088;
        static mut TLS_BUFFER: [u8; 16384] = [0; 16384];
    } else {
        use drogue_device::Address;

        const HOST: &str = "192.168.1.2";
        const PORT: u16 = 8088;
    }
}

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");
const USERNAME: &str = drogue::config!("http-username");
const PASSWORD: &str = drogue::config!("http-password");

type WAKE = Output<'static, PB13>;
type RESET = Output<'static, PE8>;
type CS = Output<'static, PE0>;
type READY = ExtiInput<'static, PE1>;
type SPI = Spi<'static, SPI3, NoDma, NoDma>; // DMA2_CH2, DMA2_CH1>;
type SpiError = spi::Error;

type EsWifi = EsWifiController<SPI, CS, RESET, WAKE, READY, SpiError>;

#[cfg(feature = "tls")]
type ConnectionFactory =
    TlsConnectionFactory<'static, AdapterActor<EsWifi>, Aes128GcmSha256, Rng<RNG>, 1>;

#[cfg(not(feature = "tls"))]
type ConnectionFactory = Address<'static, AdapterActor<EsWifi>>;

type I2cDriver = embassy_stm32::i2c::I2c<'static, I2C2, DMA1_CH4, DMA1_CH5>;

pub struct MyDevice {
    wifi: ActorContext<'static, AdapterActor<EsWifi>>,
    app: ActorContext<'static, App<ConnectionFactory>, 3>,
    i2c: ActorContext<'static, I2cPeripheral<I2cDriver>>,
    button: ActorContext<
        'static,
        Button<ExtiInput<'static, PC13>, ButtonEventDispatcher<App<ConnectionFactory>>>,
    >,
    sensor: ActorContext<
        'static,
        Sensor<ExtiInput<'static, PD15>, Address<'static, I2cPeripheral<I2cDriver>>>,
    >,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

// Clock configuration that enables suffiently fast clock for RNG
fn config() -> Config {
    let mut config = Config::default();
    config.rcc = config
        .rcc
        .clock_src(ClockSrc::PLL(
            PLLSource::HSI16,
            PLLClkDiv::Div2,
            PLLSrcDiv::Div1,
            PLLMul::Mul10,
            Some(PLLClkDiv::Div2),
        ))
        .ahb_pre(AHBPrescaler::Div8);
    config
}

#[embassy::main(config = "config()")]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
    }

    let spi = Spi::new(
        p.SPI3,
        p.PC10,
        p.PC12,
        p.PC11,
        NoDma,
        NoDma,
        //p.DMA2_CH2,
        //p.DMA2_CH1,
        Hertz(100_000),
        SpiConfig::default(),
    );

    let _boot = Output::new(p.PB12, Level::Low, Speed::VeryHigh);
    let wake = Output::new(p.PB13, Level::Low, Speed::VeryHigh);
    let reset = Output::new(p.PE8, Level::Low, Speed::VeryHigh);
    let cs = Output::new(p.PE0, Level::High, Speed::VeryHigh);
    let ready = Input::new(p.PE1, Pull::Up);
    let ready = ExtiInput::new(ready, p.EXTI1);

    let mut wifi = EsWifiController::new(spi, cs, reset, wake, ready);
    match wifi.start().await {
        Ok(()) => defmt::info!("Started..."),
        Err(err) => defmt::info!("Error... {}", err),
    }

    let button_pin = Input::new(p.PC13, Pull::Up);
    let button = ExtiInput::new(button_pin, p.EXTI13);

    let ready_pin = Input::new(p.PD15, Pull::Down);
    let sensor_ready = ExtiInput::new(ready_pin, p.EXTI15);

    let i2c_irq = interrupt::take!(I2C2_EV);
    let i2c = i2c::I2c::new(
        p.I2C2,
        p.PB10,
        p.PB11,
        i2c_irq,
        p.DMA1_CH4,
        p.DMA1_CH5,
        Hertz(100_000),
    );

    #[cfg(feature = "tls")]
    let rng = Rng::new(p.RNG);

    DEVICE.configure(MyDevice {
        wifi: ActorContext::new(AdapterActor::new()),
        button: ActorContext::new(Button::new(button)),
        app: ActorContext::new(App::new(
            HOST,
            PORT,
            USERNAME.trim_end(),
            PASSWORD.trim_end(),
        )),
        i2c: ActorContext::new(I2cPeripheral::new(i2c)),
        sensor: ActorContext::new(Sensor::new(sensor_ready)),
    });

    let (mut sensor, app) = DEVICE
        .mount(|device| async move {
            let mut wifi = device.wifi.mount(wifi, spawner);
            defmt::info!("Joining WiFi network...");
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
            let i2c = device.i2c.mount((), spawner);
            let sensor = device.sensor.mount(i2c, spawner);
            device.button.mount(app.into(), spawner);
            (sensor, app)
        })
        .await;

    defmt::info!("Application initialized. Press 'User' button to send data");

    // Adjust interval to your liking
    let interval = Duration::from_secs(30);
    loop {
        if let Ok(data) = sensor.temperature().await {
            let data = TemperatureData {
                geoloc: None,
                temp: Some(data.temperature.raw_value()),
                hum: Some(data.relative_humidity),
            };
            let _ = app.request(Command::Update(data)).unwrap().await;
        }
        let _ = app.request(Command::Send).unwrap().await;
        Timer::after(interval).await;
    }
}
