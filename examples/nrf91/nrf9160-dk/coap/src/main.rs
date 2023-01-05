#![no_std]
#![no_main]
#![macro_use]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]
#![feature(alloc_error_handler)]

use {
    alloc_cortex_m::CortexMHeap,
    coap_lite::{CoapRequest, ContentFormat, RequestType},
    core::mem::MaybeUninit,
    drogue_device::*,
    embassy_executor::Spawner,
    embassy_futures::select::{select, Either},
    embassy_nrf::{
        gpio::{Level, Output, OutputDrive},
        interrupt::{self, InterruptExt, Priority},
    },
    embassy_sync::{
        blocking_mutex::raw::ThreadModeRawMutex,
        channel::{Channel, DynamicReceiver, DynamicSender},
    },
    embassy_time::{Duration, Ticker, Timer},
    futures::StreamExt,
    heapless::Vec,
    nrf_modem::{ConnectionPreference, DtlsSocket, LteLink, PeerVerification, SystemMode},
    static_cell::StaticCell,
};

mod psk;
use psk::*;

extern crate tinyrlibc;

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

#[cfg(feature = "panic-reset")]
use panic_reset as _;

const SECURITY_TAG: u32 = 1;

const FIRMWARE_VERSION: &str = env!("CARGO_PKG_VERSION");
const FIRMWARE_REVISION: Option<&str> = option_env!("REVISION");

/// CoAP endpoint hostname
const HOSTNAME: &str = drogue::config!("hostname");

/// CoAP endpoint port
const PORT: &str = drogue::config!("port");

/// Device identity
const IDENTITY: &str = drogue::config!("username");

/// Pre-shared key
const PSK: &str = drogue::config!("password");

#[embassy_executor::main]
async fn main(_s: Spawner) {
    unsafe {
        ALLOCATOR.init(HEAP_DATA.as_ptr() as usize, HEAP_DATA.len());
    }

    let p = embassy_nrf::init(Default::default());
    let version = FIRMWARE_REVISION.unwrap_or(FIRMWARE_VERSION);
    defmt::info!("Running firmware version {}", version);

    let egu1 = interrupt::take!(EGU1);
    egu1.set_priority(Priority::P4);
    egu1.set_handler(|_| {
        nrf_modem::application_irq_handler();
        cortex_m::asm::sev();
    });
    egu1.enable();

    let ipc = interrupt::take!(IPC);
    ipc.set_priority(Priority::P0);
    ipc.set_handler(|_| {
        nrf_modem::ipc_irq_handler();
        cortex_m::asm::sev();
    });
    ipc.enable();

    let _sim_select = Output::new(p.P0_08, Level::Low, OutputDrive::Standard);

    defmt::info!("Initializing modem");
    nrf_modem::init(SystemMode {
        lte_support: true,
        lte_psm_support: true,
        nbiot_support: true,
        gnss_support: false,
        preference: ConnectionPreference::Lte,
    })
    .await
    .unwrap();

    install_psk_id_and_psk().await.unwrap();

    defmt::info!("Acquiring modem link");
    let link = LteLink::new().await.unwrap();

    defmt::info!("Waiting until connected to network");
    link.wait_for_link().await.unwrap();

    let host = HOSTNAME.trim_start();
    let port: u16 = PORT.trim_start().parse::<u16>().unwrap();

    Timer::after(Duration::from_secs(6)).await;

    loop {
        defmt::info!("Connecting to CoAP endpoint coaps://{}:{}", host, port,);

        let socket =
            nrf_modem::DtlsSocket::connect(host, port, PeerVerification::Enabled, &[SECURITY_TAG])
                .await
                .unwrap();

        defmt::info!("Connected!");
        defmt::info!("Encoding request");
        let mut request: CoapRequest<DtlsSocket> = CoapRequest::new();
        request.set_method(RequestType::Post);
        request.set_path("/v1/sensor");
        request
            .message
            .set_content_format(ContentFormat::ApplicationJSON);
        request.message.payload = "{\"temp\":22.3}".into();

        defmt::info!("Sending CoAP request");
        socket
            .send(&request.message.to_bytes().unwrap())
            .await
            .unwrap();
        defmt::info!("CoAP request sent!");
        Timer::after(Duration::from_secs(30)).await;
    }
}

#[global_allocator]
static ALLOCATOR: CortexMHeap = CortexMHeap::empty();

static mut HEAP_DATA: [MaybeUninit<u8>; 16384] = [MaybeUninit::uninit(); 16384];

/// Default alloc error handler for when allocation fails
#[alloc_error_handler]
fn alloc_error(_: core::alloc::Layout) -> ! {
    defmt::info!("Alloc error!");
    cortex_m::asm::udf()
}
