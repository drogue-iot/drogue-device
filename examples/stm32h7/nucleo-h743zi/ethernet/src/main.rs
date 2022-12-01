#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]

use {defmt_rtt as _, panic_probe as _};

use {
    core::fmt::Write,
    drogue_device::*,
    embassy_net::{
        tcp::client::{TcpClient, TcpClientState},
        Stack, StackResources,
    },
    embassy_time::{Duration, Timer},
    heapless::String,
    nucleo_h743zi::*,
    rand_core::RngCore,
    reqwless::{
        client::{HttpClient, TlsConfig},
        request::{ContentType, Method},
    },
    static_cell::StaticCell,
};

#[path = "../../../../common/dns.rs"]
mod dns;
use dns::*;

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

static RESOURCES: StaticCell<StackResources<1, 2, 8>> = StaticCell::new();
static STACK: StaticCell<Stack<EthernetDevice>> = StaticCell::new();

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<EthernetDevice>) -> ! {
    stack.run().await
}

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let mut board = NucleoH743::default();

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

    let mut url: String<128> = String::new();
    write!(
        url,
        "https://{}:{}/v1/temperature?data_schema=urn:drogue:iot:temperature",
        HOSTNAME, PORT
    )
    .unwrap();

    let mut tls = [0; 8000];
    let mut client = HttpClient::new_with_tls(&network, &DNS, TlsConfig::new(seed, &mut tls));

    defmt::info!("Application initialized. Press the blue button to send data");
    loop {
        // Wait until we have a sensor reading
        board.user_button.wait_for_any_edge().await;

        let sensor_data = TemperatureData {
            geoloc: None,
            temp: Some(22.2),
            hum: None,
        };

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
        Timer::after(Duration::from_secs(2)).await;
    }
}
