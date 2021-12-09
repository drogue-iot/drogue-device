use crate::bsp::Board;
use crate::drivers::button::Button;
use crate::drivers::led::{ActiveHigh, Led};
use embassy::util::Forever;
use embassy_stm32::eth::lan8742a::LAN8742A;
use embassy_stm32::eth::{Ethernet, State};
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::interrupt;
use embassy_stm32::peripherals::{PB0, PB14, PC13, PE1, RNG};
use embassy_stm32::rng::Rng as HalRng;

pub type PinLedRed = Output<'static, PB14>;
pub type LedRed = Led<PinLedRed, ActiveHigh>;

pub type PinLedGreen = Output<'static, PB0>;
pub type LedGreen = Led<PinLedGreen, ActiveHigh>;

pub type PinLedYellow = Output<'static, PE1>;
pub type LedYellow = Led<PinLedYellow, ActiveHigh>;

pub type PinUserButton = Input<'static, PC13>;
pub type UserButton = Button<ExtiInput<'static, PC13>>;

pub type EthernetDevice = Ethernet<'static, LAN8742A, 4, 4>;

pub type Rng = HalRng<RNG>;

pub struct NucleoH743 {
    pub led_red: LedRed,
    pub led_green: LedGreen,
    pub led_yellow: LedYellow,
    pub user_button: UserButton,
    pub eth: EthernetDevice,
    pub rng: Rng,
}

impl Board for NucleoH743 {
    type Peripherals = embassy_stm32::Peripherals;

    fn new(p: Self::Peripherals) -> Self {
        static ETH_STATE: Forever<State<'static, 4, 4>> = Forever::new();
        let eth_int = interrupt::take!(ETH);
        let mac_addr = [0x10; 6];
        let state = ETH_STATE.put(State::new());
        let eth = unsafe {
            Ethernet::new(
                state, p.ETH, eth_int, p.PA1, p.PA2, p.PC1, p.PA7, p.PC4, p.PC5, p.PG13, p.PB13,
                p.PG11, LAN8742A, mac_addr, 0,
            )
        };
        Self {
            led_red: Led::new(Output::new(p.PB14, Level::Low, Speed::Low)),
            led_green: Led::new(Output::new(p.PB0, Level::Low, Speed::Low)),
            led_yellow: Led::new(Output::new(p.PE1, Level::Low, Speed::Low)),
            user_button: Button::new(ExtiInput::new(Input::new(p.PC13, Pull::Down), p.EXTI13)),
            eth,
            rng: Rng::new(p.RNG),
        }
    }
}
