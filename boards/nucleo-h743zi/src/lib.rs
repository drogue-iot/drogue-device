#![no_std]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]
#[allow(unused_imports)]
use embassy_stm32::interrupt;
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    peripherals::{PB0, PB14, PC13, PE1, RNG},
    rng::Rng as HalRng,
    Config,
};
#[allow(unused_imports)]
use static_cell::StaticCell;

use embassy_stm32::{
    eth::{generic_smi::GenericSMI, Ethernet, PacketQueue},
    peripherals::ETH,
};

pub type LedRed = Output<'static, PB14>;
pub type LedGreen = Output<'static, PB0>;
pub type LedYellow = Output<'static, PE1>;
pub type UserButton = ExtiInput<'static, PC13>;

pub type EthernetDevice = Ethernet<'static, ETH, GenericSMI>;

pub type Rng = HalRng<'static, RNG>;

pub struct NucleoH743 {
    pub led_red: LedRed,
    pub led_green: LedGreen,
    pub led_yellow: LedYellow,
    pub user_button: UserButton,
    pub eth: EthernetDevice,
    pub rng: Rng,
}

impl Default for NucleoH743 {
    fn default() -> Self {
        let mut config = Config::default();
        config.enable_debug_during_sleep = true;
        Self::new(config)
    }
}

impl NucleoH743 {
    fn new(config: Config) -> Self {
        let p = embassy_stm32::init(config);
        static PQUEUE: StaticCell<PacketQueue<16, 16>> = StaticCell::new();
        let eth_int = interrupt::take!(ETH);
        let mac_addr = [0x10; 6];
        let pqueue = PQUEUE.init(PacketQueue::new());
        let eth = Ethernet::new(
            pqueue, p.ETH, eth_int, p.PA1, p.PA2, p.PC1, p.PA7, p.PC4, p.PC5, p.PG13, p.PB13,
            p.PG11, GenericSMI, mac_addr, 0,
        );
        Self {
            led_red: Output::new(p.PB14, Level::Low, Speed::Low),
            led_green: Output::new(p.PB0, Level::Low, Speed::Low),
            led_yellow: Output::new(p.PE1, Level::Low, Speed::Low),
            user_button: ExtiInput::new(Input::new(p.PC13, Pull::Down), p.EXTI13),
            eth,
            rng: Rng::new(p.RNG),
        }
    }
}
