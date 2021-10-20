#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;

use core::future::Future;
use core::pin::Pin;
use drogue_device::actors::button::{ButtonEvent, FromButtonEvent};
use drogue_device::actors::tcp::smoltcp::SmolTcp;
use drogue_device::actors::led::{Led, LedMessage};
use drogue_device::drivers::tcp::smoltcp::SmolTcpStack;
use drogue_device::{
    actors::button::Button, Actor, ActorContext, Address, DeviceContext, Inbox, Package,
};
use embassy::util::Forever;
//use embassy_macros::interrupt_take;
use embassy_net::StaticConfigurator;
use embassy_net::{Config as NetConfig, Ipv4Address, Ipv4Cidr, StackResources, TcpSocket};
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::eth::lan8742a::LAN8742A;
use embassy_stm32::interrupt;
use embassy_stm32::peripherals::{PB0, PB14, PC13, PE1, RNG};
use embassy_stm32::rng::Rng;
use embassy_stm32::{
    eth::{Ethernet, State},
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    Peripherals,
};
use heapless::Vec;

type LedYellowPin = Output<'static, PE1>;

type EthernetDevice = Ethernet<'static, LAN8742A, 4, 4>;
type SmolTcpPackage = SmolTcp<EthernetDevice, StaticConfigurator, 1, 2, 1024>;

static STATE: Forever<State<'static, 4, 4>> = Forever::new();

pub struct MyDevice {
    tcp: SmolTcpPackage,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

static mut RNG_INST: Option<Rng<RNG>> = None;

#[no_mangle]
fn _embassy_rand(buf: &mut [u8]) {
    use rand_core::RngCore;

    critical_section::with(|_| unsafe {
        defmt::unwrap!(RNG_INST.as_mut()).fill_bytes(buf);
    });
}

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
    }

    let rng = Rng::new(p.RNG);
    unsafe {
        RNG_INST.replace(rng);
    }

    let eth_int = interrupt::take!(ETH);
    let mac_addr = [0x10; 6];
    let state = STATE.put(State::new());
    let eth = unsafe {
        Ethernet::new(
            state, p.ETH, eth_int, p.PA1, p.PA2, p.PC1, p.PA7, p.PC4, p.PC5, p.PG13, p.PB13,
            p.PG11, LAN8742A, mac_addr, 0,
        )
    };

    DEVICE.configure(MyDevice {
        tcp: SmolTcpPackage::new(eth),
    });

    let config = StaticConfigurator::new(NetConfig {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 0, 111), 24),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(192, 168, 0, 1)),
    });

    DEVICE
        .mount(|device| async move { device.tcp.mount(config, spawner) })
        .await;
    //defmt::info!("Application initialized. Press 'A' button to cycle LEDs");
}
