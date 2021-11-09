#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    actors::net::*,
    actors::{button::*, tcp::smoltcp::SmolTcp},
    domain::{temperature::Temperature, SensorAcquisition},
    ActorContext, DeviceContext, Package,
};
use drogue_temperature::*;
use drogue_tls::Aes128GcmSha256;
use embassy::util::Forever;
use embassy_net::StaticConfigurator;
use embassy_net::{Config as NetConfig, Ipv4Address, Ipv4Cidr};
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::eth::lan8742a::LAN8742A;
use embassy_stm32::interrupt;
use embassy_stm32::peripherals::RNG;
use embassy_stm32::rng::Rng;
use embassy_stm32::{
    eth::{Ethernet, State},
    Peripherals,
};
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{Input, Pull},
    peripherals::PC13,
};
use heapless::Vec;

//const HOST: &str = "http.sandbox.drogue.cloud";
//const IP: IpAddress = IpAddress::new_v4(95, 216, 224, 167); // IP resolved for "http.sandbox.drogue.cloud"
//const PORT: u16 = 443;

const HOST: &str = "localhost";
const PORT: u16 = 8088;

const USERNAME: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.username.txt"));
const PASSWORD: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.password.txt"));

type EthernetDevice = Ethernet<'static, LAN8742A, 4, 4>;
type SmolTcpPackage = SmolTcp<EthernetDevice, StaticConfigurator, 1, 2, 1024>;

type ConnectionFactory = TlsConnectionFactory<
    'static,
    <SmolTcpPackage as Package>::Primary,
    Aes128GcmSha256,
    TlsRand,
    1,
>;
static mut TLS_BUFFER: [u8; 16384] = [0; 16384];

static STATE: Forever<State<'static, 4, 4>> = Forever::new();

pub struct MyDevice {
    tcp: SmolTcpPackage,
    app: ActorContext<'static, App<ConnectionFactory>, 2>,
    button:
        ActorContext<'static, Button<'static, ExtiInput<'static, PC13>, App<ConnectionFactory>>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

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

    let button = Input::new(p.PC13, Pull::Down);
    let button = ExtiInput::new(button, p.EXTI13);

    DEVICE.configure(MyDevice {
        tcp: SmolTcpPackage::new(eth),
        app: ActorContext::new(App::new(
            HOST,
            PORT,
            USERNAME.trim_end(),
            PASSWORD.trim_end(),
        )),
        button: ActorContext::new(Button::new(button)),
    });

    let config = StaticConfigurator::new(NetConfig {
        address: Ipv4Cidr::new(Ipv4Address::new(192, 168, 0, 111), 24),
        dns_servers: Vec::new(),
        gateway: Some(Ipv4Address::new(192, 168, 0, 1)),
    });

    DEVICE
        .mount(|device| async move {
            let net = device.tcp.mount(config, spawner);
            let factory = TlsConnectionFactory::new(net, TlsRand, [unsafe { &mut TLS_BUFFER }; 1]);

            let app = device.app.mount(factory, spawner);
            app.request(Command::Update(SensorData {
                data: SensorAcquisition {
                    temperature: Temperature::new(22.0),
                    relative_humidity: 0.0,
                },
                location: None,
            }))
            .unwrap()
            .await;
            device.button.mount(app, spawner);
        })
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
