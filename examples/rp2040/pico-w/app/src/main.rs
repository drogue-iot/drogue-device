#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]

use {
    core::{convert::Infallible, fmt::Write as _},
    defmt_rtt as _,
    drogue_device::{
        firmware::FirmwareManager,
        ota::{ota_task, OtaConfig},
        *,
    },
    embassy_executor::Spawner,
    embassy_net::{
        dns::DnsSocket,
        tcp::client::{TcpClient, TcpClientState},
        Stack, StackResources,
    },
    embassy_rp::{
        flash::Flash,
        gpio::{Flex, Level, Output},
        interrupt,
        peripherals::{FLASH, PIN_23, PIN_24, PIN_25, PIN_29, USB},
        usb::Driver,
    },
    embassy_time::{Duration, Timer},
    embedded_hal_1::spi::ErrorType,
    embedded_hal_async::spi::{ExclusiveDevice, SpiBusFlush, SpiBusRead, SpiBusWrite},
    embedded_nal_async::*,
    heapless::String,
    panic_probe as _,
    reqwless::{
        client::{HttpClient, TlsConfig, TlsVerify},
        headers::ContentType,
        request::{Method, RequestBuilder},
    },
    static_cell::StaticCell,
};

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

mod fmt;

#[path = "../../../../common/temperature.rs"]
mod temperature;
use temperature::*;

/// HTTP endpoint hostname
const HOSTNAME: &str = drogue::config!("hostname");

/// HTTP endpoint port
const PORT: &str = drogue::config!("port");

/// HTTP username
const USERNAME: &str = drogue::config!("username");

/// HTTP password
const PASSWORD: &str = drogue::config!("password");

const FLASH_SIZE: usize = 2 * 1024 * 1024;
const FIRMWARE_VERSION: &str = env!("CARGO_PKG_VERSION");
const FIRMWARE_REVISION: Option<&str> = option_env!("REVISION");

type NetDriver = cyw43::NetDriver<'static>;

#[embassy_executor::task]
async fn wifi_task(
    runner: cyw43::Runner<
        'static,
        Output<'static, PIN_23>,
        ExclusiveDevice<MySpi, Output<'static, PIN_25>>,
    >,
) -> ! {
    runner.run().await
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<NetDriver>) -> ! {
    stack.run().await
}

#[embassy_executor::task]
async fn logger_task(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Info, driver);
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let irq = interrupt::take!(USBCTRL_IRQ);
    let driver = Driver::new(p.USB, irq);
    spawner.spawn(logger_task(driver)).unwrap();

    // NOTE: Download firmware from https://github.com/embassy-rs/cyw43/tree/master/firmware
    // to run this example, and flash them with probe-rs-cli:
    //
    // probe-rs-cli download 43439A0.bin --format bin --chip RP2040 --base-address 0x10100000
    // probe-rs-cli download 43439A0.clm_blob --format bin --chip RP2040 --base-address 0x10140000
    let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 224190) };
    let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };
    // let fw = include_bytes!("../firmware/43439A0.bin");
    // let clm = include_bytes!("../firmware/43439A0_clm.bin");

    let pwr = Output::new(p.PIN_23, Level::Low);
    let cs = Output::new(p.PIN_25, Level::High);
    let clk = Output::new(p.PIN_29, Level::Low);
    let mut dio = Flex::new(p.PIN_24);
    dio.set_low();
    dio.set_as_output();

    let bus = MySpi { clk, dio };
    let spi = ExclusiveDevice::new(bus, cs);

    static STATE: StaticCell<cyw43::State> = StaticCell::new();
    let state = STATE.init(cyw43::State::new());
    let (net_device, mut control, runner) = cyw43::new(state, pwr, spi, fw).await;

    spawner.spawn(wifi_task(runner)).unwrap();

    control.init(clm).await;
    control
        .set_power_management(cyw43::PowerManagementMode::PowerSave)
        .await;

    control
        .join_wpa2(WIFI_SSID.trim_end(), WIFI_PSK.trim_end())
        .await;

    let config = embassy_net::Config::Dhcp(Default::default());

    // Generate random seed
    let seed = 0x0123_4567_89ab_cdef; // chosen by fair dice roll. guarenteed to be random.

    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let resources = RESOURCES.init(StackResources::new());

    static STACK: StaticCell<Stack<NetDriver>> = StaticCell::new();
    let stack = STACK.init(Stack::new(net_device, config, resources, seed));

    unwrap!(spawner.spawn(net_task(stack)));

    static CLIENT_STATE: TcpClientState<1, 1024, 1024> = TcpClientState::new();
    let client = TcpClient::new(&stack, &CLIENT_STATE);

    // Launch updater task
    spawner
        .spawn(updater_task(stack, Flash::new(p.FLASH), seed))
        .unwrap();

    let mut url: String<128> = String::new();
    write!(url, "https://{}:{}/v1/pico", HOSTNAME, PORT).unwrap();

    let mut tls_rx = [0; 16384];
    let mut tls_tx = [0; 1024];
    let dns = DnsSocket::new(stack);
    let mut client = HttpClient::new_with_tls(
        &client,
        &dns,
        TlsConfig::new(seed as u64, &mut tls_rx, &mut tls_tx, TlsVerify::None),
    );

    info!("Application initialized.");
    loop {
        Timer::after(Duration::from_secs(30)).await;
        let sensor_data = TemperatureData {
            geoloc: None,
            temp: Some(22.2),
            hum: None,
        };

        info!("Reporting sensor data: {:?}", sensor_data.temp);

        let tx: String<128> = serde_json_core::ser::to_string(&sensor_data).unwrap();
        let mut rx_buf = [0; 1024];
        let mut req = client
            .request(Method::POST, &url)
            .await
            .unwrap()
            .basic_auth(USERNAME.trim_end(), PASSWORD.trim_end())
            .body(tx.as_bytes())
            .content_type(ContentType::ApplicationJson);
        let response = req.send(&mut rx_buf[..]).await;

        match response {
            Ok(response) => {
                info!("Response status: {:?}", response.status);
                if let Ok(body) = response.body() {
                    if let Ok(payload) = body.read_to_end().await {
                        let _s = core::str::from_utf8(payload).unwrap();
                    }
                }
            }
            Err(e) => {
                warn!("Error doing HTTP request: {:?}", e);
            }
        }
        info!("Telemetry reported successfully");
    }
}

#[embassy_executor::task]
async fn updater_task(
    stack: &'static Stack<NetDriver>,
    flash: Flash<'static, FLASH, FLASH_SIZE>,
    seed: u64,
) {
    use {drogue_device::firmware::BlockingFlash, embassy_time::Timer};

    static CLIENT_STATE: TcpClientState<1, 1024, 1024> = TcpClientState::new();
    let client = TcpClient::new(&stack, &CLIENT_STATE);

    let dns = DnsSocket::new(stack);

    let version = FIRMWARE_REVISION.unwrap_or(FIRMWARE_VERSION);
    defmt::info!("Running firmware version {}", version);
    let updater = embassy_boot_rp::FirmwareUpdater::default();

    let device: FirmwareManager<BlockingFlash<Flash<'static, FLASH, FLASH_SIZE>>, 1, 2048> =
        FirmwareManager::new(BlockingFlash::new(flash), updater, version.as_bytes());

    let config = OtaConfig {
        hostname: HOSTNAME.trim_end(),
        port: PORT.parse::<u16>().unwrap(),
        username: USERNAME.trim_end(),
        password: PASSWORD.trim_end(),
    };

    Timer::after(Duration::from_secs(5)).await;
    ota_task(client, &dns, device, seed, config, || {
        cortex_m::peripheral::SCB::sys_reset()
    })
    .await
}

///////////////////////////////////////////////////////////////////////
// WIFI SPI setup
///////////////////////////////////////////////////////////////////////

struct MySpi {
    /// SPI clock
    clk: Output<'static, PIN_29>,

    /// 4 signals, all in one!!
    /// - SPI MISO
    /// - SPI MOSI
    /// - IRQ
    /// - strap to set to gSPI mode on boot.
    dio: Flex<'static, PIN_24>,
}

impl ErrorType for MySpi {
    type Error = Infallible;
}

impl SpiBusFlush for MySpi {
    async fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl SpiBusRead<u32> for MySpi {
    async fn read(&mut self, words: &mut [u32]) -> Result<(), Self::Error> {
        self.dio.set_as_input();
        for word in words {
            let mut w = 0;
            for _ in 0..32 {
                w = w << 1;

                // rising edge, sample data
                if self.dio.is_high() {
                    w |= 0x01;
                }
                self.clk.set_high();

                // falling edge
                self.clk.set_low();
            }
            *word = w
        }

        Ok(())
    }
}

impl SpiBusWrite<u32> for MySpi {
    async fn write(&mut self, words: &[u32]) -> Result<(), Self::Error> {
        self.dio.set_as_output();
        for word in words {
            let mut word = *word;
            for _ in 0..32 {
                // falling edge, setup data
                self.clk.set_low();
                if word & 0x8000_0000 == 0 {
                    self.dio.set_low();
                } else {
                    self.dio.set_high();
                }

                // rising edge
                self.clk.set_high();

                word = word << 1;
            }
        }
        self.clk.set_low();

        self.dio.set_as_input();
        Ok(())
    }
}
