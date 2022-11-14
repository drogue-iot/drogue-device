#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use core::{convert::Infallible, future::Future};

use {
    core::fmt::Write as _,
    defmt_rtt as _,
    drogue_device::*,
    embassy_executor::Spawner,
    embassy_net::{
        tcp::client::{TcpClient, TcpClientState},
        Stack, StackResources,
    },
    embassy_rp::{
        gpio::{Flex, Level, Output},
        peripherals::{PIN_23, PIN_24, PIN_25, PIN_29},
    },
    embassy_time::{Duration, Timer},
    embedded_hal_1::spi::ErrorType,
    embedded_hal_async::spi::{ExclusiveDevice, SpiBusFlush, SpiBusRead, SpiBusWrite},
    embedded_nal_async::*,
    heapless::String,
    panic_probe as _,
    rand::SeedableRng,
    rand_chacha::ChaCha8Rng,
    reqwless::{
        client::{HttpClient, TlsConfig},
        request::{ContentType, Method},
    },
    static_cell::StaticCell,
};

const WIFI_SSID: &str = drogue::config!("wifi-ssid");
const WIFI_PSK: &str = drogue::config!("wifi-password");

#[path = "../../../common/dns.rs"]
mod dns;
use dns::*;

#[path = "../../../common/temperature.rs"]
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
async fn net_task(stack: &'static Stack<cyw43::NetDevice<'static>>) -> ! {
    stack.run().await
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // NOTE: Download firmware from https://github.com/embassy-rs/cyw43/tree/master/firmware
    // to run this example, and flash them with probe-rs-cli:
    //
    // probe-rs-cli download 43439A0.bin --format bin --chip RP2040 --base-address 0x10100000
    // probe-rs-cli download 43439A0.clm_blob --format bin --chip RP2040 --base-address 0x10140000
    let fw = unsafe { core::slice::from_raw_parts(0x10100000 as *const u8, 224190) };
    let clm = unsafe { core::slice::from_raw_parts(0x10140000 as *const u8, 4752) };

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
    let (mut control, runner) = cyw43::new(state, pwr, spi, fw).await;

    spawner.spawn(wifi_task(runner)).unwrap();

    let net_device = control.init(clm).await;

    control
        .join_wpa2(WIFI_SSID.trim_end(), WIFI_PSK.trim_end())
        .await;

    let config = embassy_net::ConfigStrategy::Dhcp;

    // Generate random seed
    let seed = 0x0123_4567_89ab_cdef; // chosen by fair dice roll. guarenteed to be random.

    static RESOURCES: StaticCell<StackResources<1, 2, 8>> = StaticCell::new();
    let resources = RESOURCES.init(StackResources::new());

    static STACK: StaticCell<Stack<cyw43::NetDevice<'static>>> = StaticCell::new();
    let stack = STACK.init(Stack::new(net_device, config, resources, seed));

    defmt::unwrap!(spawner.spawn(net_task(stack)));

    static CLIENT_STATE: TcpClientState<1, 1024, 1024> = TcpClientState::new();
    let client = TcpClient::new(&stack, &CLIENT_STATE);

    let mut url: String<128> = String::new();
    write!(url, "https://{}:{}/v1/pico", HOSTNAME, PORT).unwrap();

    let mut rng = ChaCha8Rng::seed_from_u64(seed as u64);
    let mut tls = [0; 16384];
    let mut client = HttpClient::new_with_tls(&client, &DNS, TlsConfig::new(&mut rng, &mut tls));

    defmt::info!("Application initialized.");
    loop {
        Timer::after(Duration::from_secs(30)).await;
        let sensor_data = TemperatureData {
            geoloc: None,
            temp: Some(22.2),
            hum: None,
        };
        defmt::info!(
            "Reporting sensor data: {:?}",
            defmt::Debug2Format(&sensor_data)
        );

        let tx: String<128> = serde_json_core::ser::to_string(&sensor_data).unwrap();
        let mut rx_buf = [0; 1024];
        let response = client
            .request(Method::POST, &url)
            .await
            .unwrap()
            .basic_auth(USERNAME.trim_end(), PASSWORD.trim_end())
            .body(tx.as_bytes())
            .content_type(ContentType::ApplicationJson)
            .send(&mut rx_buf[..])
            .await;

        match response {
            Ok(response) => {
                defmt::info!("Response status: {:?}", response.status);
                if let Some(payload) = response.body {
                    let _s = core::str::from_utf8(payload).unwrap();
                }
            }
            Err(e) => {
                defmt::warn!("Error doing HTTP request: {:?}", e);
            }
        }
        defmt::info!("Telemetry reported successfully");
    }
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
    type FlushFuture<'a> = impl Future<Output = Result<(), Self::Error>>
    where
        Self: 'a;

    fn flush<'a>(&'a mut self) -> Self::FlushFuture<'a> {
        async move { Ok(()) }
    }
}

impl SpiBusRead<u32> for MySpi {
    type ReadFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a
    where
        Self: 'a;

    fn read<'a>(&'a mut self, words: &'a mut [u32]) -> Self::ReadFuture<'a> {
        async move {
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
}

impl SpiBusWrite<u32> for MySpi {
    type WriteFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a
    where
        Self: 'a;

    fn write<'a>(&'a mut self, words: &'a [u32]) -> Self::WriteFuture<'a> {
        async move {
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
}
