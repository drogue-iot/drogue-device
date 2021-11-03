#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    actors::{button::*, socket::*, tcp::smoltcp::SmolTcp},
    domain::{temperature::Temperature, SensorAcquisition},
    traits::ip::*,
    ActorContext, DeviceContext, Package,
};
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
use drogue_temperature::*;

use drogue_device::actors::socket::TlsSocket;
use drogue_tls::{Aes128GcmSha256, TlsContext};

//const HOST: &str = "http.sandbox.drogue.cloud";
//const IP: IpAddress = IpAddress::new_v4(95, 216, 224, 167); // IP resolved for "http.sandbox.drogue.cloud"
//const PORT: u16 = 443;

const HOST: &str = "localhost";
const IP: IpAddress = IpAddress::new_v4(192, 168, 1, 2); // IP of local drogue service
const PORT: u16 = 8088;
static mut TLS_BUFFER: [u8; 16384] = [0u8; 16384];

const USERNAME: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.username.txt"));
const PASSWORD: &str = include_str!(concat!(env!("OUT_DIR"), "/config/http.password.txt"));

type EthernetDevice = Ethernet<'static, LAN8742A, 4, 4>;
type SmolTcpPackage = SmolTcp<EthernetDevice, StaticConfigurator, 1, 2, 1024>;

type AppSocket = TlsSocket<
    'static,
    Socket<'static, <SmolTcpPackage as Package>::Primary>,
    TlsRand,
    Aes128GcmSha256,
>;

static STATE: Forever<State<'static, 4, 4>> = Forever::new();

pub struct MyDevice {
    tcp: SmolTcpPackage,
    app: ActorContext<'static, App<AppSocket>, 2>,
    button: ActorContext<'static, Button<'static, ExtiInput<'static, PC13>, App<AppSocket>>>,
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
        app: ActorContext::new(App::new(IP, PORT, USERNAME.trim_end(), PASSWORD.trim_end())),
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
            let socket = Socket::new(net, net.open().await.unwrap());
            let socket = TlsSocket::wrap(
                socket,
                TlsContext::new(TlsRand, unsafe { &mut TLS_BUFFER })
                    .with_server_name(HOST.trim_end()),
            );

            let app = device.app.mount(socket, spawner);
            app.request(Command::Update(SensorAcquisition {
                temperature: Temperature::new(22.0),
                relative_humidity: 0.0,
            }))
            .unwrap()
            .await;
            device.button.mount(app, spawner);
        })
        .await;
    defmt::info!("Application initialized. Press 'A' button to send data");
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
