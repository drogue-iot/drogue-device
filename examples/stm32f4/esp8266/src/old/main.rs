#![no_main]
#![no_std]

// Network-specfic values
const WIFI_SSID: &str = include_str!("wifi.ssid.txt");
const WIFI_PASSWORD: &str = include_str!("wifi.password.txt");
const ENDPOINT: &str = "192.168.0.115";
const ENDPOINT_PORT: u16 = 8080;

mod device;

use core::str::from_utf8;
use drogue_tls::{
    entropy::StaticEntropySource,
    net::tcp_stack::SslTcpStack,
    platform::SslPlatform,
    ssl::config::{Preset, Transport, Verify},
};
use heapless::consts::{U1024, U512};

use log::{info, LevelFilter};
use panic_rtt_target as _;
use rtt_logger::RTTLogger;
use rtt_target::rtt_init_print;

use rtic::app;
use rtic::cyccnt::U32Ext;

use drogue_esp8266::{ingress::Ingress, protocol::WiFiMode};
use drogue_http_client::{tcp::TcpSocketSinkSource, BufferResponseHandler, HttpConnection, Source};
use drogue_network::{
    addr::HostSocketAddr,
    dns::{AddrType, Dns},
    tcp::{Mode, TcpStack},
};

static LOGGER: RTTLogger = RTTLogger::new(LevelFilter::Info);
const DIGEST_DELAY: u32 = 200;

#[app(device = nucleo_f401re::pac, peripherals = true, monotonic = rtic::cyccnt::CYCCNT)]
const APP: () = {
    struct Resources {
        adapter: Option<device::ESPAdapter>,
        ingress: Ingress<'static, device::SerialRx>,
    }

    #[init(spawn = [digest])]
    fn init(ctx: init::Context) -> init::LateResources {
        rtt_init_print!(BlockIfFull, 2048);
        log::set_logger(&LOGGER).unwrap();
        log::set_max_level(log::LevelFilter::Trace);

        // Enable CYCNT
        let mut cmp = ctx.core;
        cmp.DWT.enable_cycle_counter();

        let (adapter, ingress) = device::network_adapter(ctx.device);

        ctx.spawn.digest().unwrap();

        info!("initialized");

        init::LateResources {
            adapter: Some(adapter),
            ingress,
        }
    }

    #[task(schedule = [digest], priority = 2, resources = [ingress])]
    fn digest(mut ctx: digest::Context) {
        ctx.resources.ingress.lock(|ingress| ingress.digest());
        ctx.schedule
            .digest(ctx.scheduled + (DIGEST_DELAY * 100_000).cycles())
            .unwrap();
    }

    #[task(binds = USART6, priority = 10, resources = [ingress])]
    fn usart(ctx: usart::Context) {
        if let Err(b) = ctx.resources.ingress.isr() {
            info!("failed to ingress {}", b as char);
        }
    }

    #[idle(resources = [adapter])]
    fn idle(ctx: idle::Context) -> ! {
        info!("idle");

        let mut adapter: device::ESPAdapter = ctx.resources.adapter.take().unwrap();

        let result = adapter.get_firmware_info();
        info!("firmware: {:?}", result);

        let result = adapter.set_mode(WiFiMode::Station);
        info!("set mode {:?}", result);

        let result = adapter.join(WIFI_SSID, WIFI_PASSWORD);
        info!("joined wifi {:?}", result);

        let result = adapter.get_ip_address();
        info!("IP {:?}", result);

        let network = adapter.into_network_stack();
        info!("network intialized");

        let addr = network.gethostbyname(ENDPOINT, AddrType::IPv4).unwrap();
        info!("Resolve IP address to {:?}", addr);

        // BEGIN SSL-ify!
        let mut ssl_platform =
            SslPlatform::setup(cortex_m_rt::heap_start() as usize, 1024 * 64).unwrap();

        ssl_platform
            .entropy_context_mut()
            .add_source(StaticEntropySource);

        ssl_platform.seed_rng().unwrap();

        let mut ssl_config = ssl_platform
            .new_client_config(Transport::Stream, Preset::Default)
            .unwrap();
        ssl_config.authmode(Verify::None);

        // consume the config, take a non-mutable ref to the underlying network.
        let mut secure_network = SslTcpStack::new(ssl_config, &network);
        // END SSL-ify!

        let socket = secure_network.open(Mode::Blocking).unwrap();
        info!("socket {:?}", socket);

        let socket_addr = HostSocketAddr::new(addr, ENDPOINT_PORT);

        let mut socket = secure_network.connect(socket, socket_addr).unwrap();

        info!("socket connected {:?}", result);

        let mut tcp = TcpSocketSinkSource::from(&mut secure_network, &mut socket);

        let con = HttpConnection::<U1024>::new();

        // dummy test data
        let data = r#"{"temp": 41.23}"#;

        let handler = BufferResponseHandler::<U1024>::new();

        log::info!("Starting request...");

        let mut req = con
            .post("/v1/anything")
            .headers(&[
                ("Host", ENDPOINT),
                ("Content-Type", "text/json"),
                ("Authorization", "Basic ZGV2aWNlX2lkQGFwcF9pZDpmb29iYXI="),
            ])
            .handler(handler)
            .execute_with::<_, U512>(&mut tcp, Some(data.as_bytes()));

        log::info!("Request sent, piping data...");

        tcp.pipe_data(&mut req).unwrap();

        log::info!("Done piping data, checking result");

        let (_, handler) = req.complete();

        log::info!(
            "Result: {} {}, Payload: {:?}",
            handler.code(),
            handler.reason(),
            from_utf8(handler.payload())
        );

        loop {
            continue;
        }
    }

    // spare interrupt used for scheduling software tasks
    extern "C" {
        fn SPI1();
        fn SPI2();
    }
};
