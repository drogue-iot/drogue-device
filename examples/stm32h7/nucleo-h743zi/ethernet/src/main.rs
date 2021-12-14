#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    actors::tcp::smoltcp::SmolTcp,
    bind_bsp,
    bsp::{boards::stm32h7::nucleo_h743zi::*, Board},
    domain::temperature::Celsius,
    DeviceContext, Package,
};
use drogue_temperature::*;
use embassy_net::StaticConfigurator;
use embassy_net::{Config as NetConfig, Ipv4Address, Ipv4Cidr};
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::eth::lan8742a::LAN8742A;
use embassy_stm32::peripherals::RNG;
use embassy_stm32::rng::Rng;
use embassy_stm32::{eth::Ethernet, Peripherals};
use heapless::Vec;

type EthernetDevice = Ethernet<'static, LAN8742A, 4, 4>;
type SmolTcpPackage = SmolTcp<EthernetDevice, StaticConfigurator, 1, 2, 1024>;

// Creates a newtype named `BSP` around the `NucleoH743` to avoid
// orphan rules and apply delegation boilerplate.
bind_bsp!(NucleoH743, BSP);

impl TemperatureBoard for BSP {
    type NetworkPackage = SmolTcpPackage;
    type Network = <SmolTcpPackage as Package>::Primary;
    type TemperatureScale = Celsius;
    type SensorReadyIndicator = AlwaysReady;
    type Sensor = FakeSensor;
    type SendTrigger = UserButton;
    type Rng = TlsRand;
}

static DEVICE: DeviceContext<TemperatureDevice<BSP>> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
    }

    let board = NucleoH743::new(p);

    unsafe {
        RNG_INST.replace(board.rng);
    }

    let config = StaticConfigurator::new(NetConfig {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 0, 111), 24),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(192, 168, 0, 1)),
    });

    DEVICE
        .configure(TemperatureDevice::new(SmolTcpPackage::new(
            board.eth, config,
        )))
        .mount(
            spawner,
            TlsRand,
            TemperatureBoardConfig {
                send_trigger: board.user_button,
                sensor: FakeSensor(22.0),
                sensor_ready: AlwaysReady,
                network_config: (),
            },
        )
        .await;
    defmt::info!("Application initialized. Press the blue button to send data");
}

static mut RNG_INST: Option<Rng<RNG>> = None;

#[no_mangle]
fn _embassy_rand(buf: &mut [u8]) {
    use rand_core::RngCore;

    critical_section::with(|_| unsafe {
        defmt::unwrap!(RNG_INST.as_mut()).fill_bytes(buf);
    });
}

pub struct TlsRand;

impl rand_core::RngCore for TlsRand {
    fn next_u32(&mut self) -> u32 {
        critical_section::with(|_| unsafe { defmt::unwrap!(RNG_INST.as_mut()).next_u32() })
    }
    fn next_u64(&mut self) -> u64 {
        critical_section::with(|_| unsafe { defmt::unwrap!(RNG_INST.as_mut()).next_u64() })
    }
    fn fill_bytes(&mut self, buf: &mut [u8]) {
        critical_section::with(|_| unsafe {
            defmt::unwrap!(RNG_INST.as_mut()).fill_bytes(buf);
        });
    }
    fn try_fill_bytes(&mut self, buf: &mut [u8]) -> Result<(), rand_core::Error> {
        critical_section::with(|_| unsafe {
            defmt::unwrap!(RNG_INST.as_mut()).fill_bytes(buf);
        });
        Ok(())
    }
}
impl rand_core::CryptoRng for TlsRand {}
