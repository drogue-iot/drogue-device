use crate::device::Device;
use embassy_stm32::{
    dma::NoDma,
    exti::{Channel, ExtiInput},
    //flash::Flash,
    gpio::{AnyPin, Input, Level, Output, Pin, Pull, Speed},
    i2c::I2c,
    interrupt,
    peripherals::{
        DMA1_CH2, DMA1_CH3, DMA1_CH4, DMA1_CH5, DMA1_CH6, DMA1_CH7, DMA2_CH1, DMA2_CH2, I2C1, I2C2,
        PA5, PB13, PB14, PC13, PD15, PE0, PE1, PE8, RNG, SPI1, SPI3,
    },
    rcc::{AHBPrescaler, ClockSrc, PLLClkDiv, PLLMul, PLLSource, PLLSrcDiv},
    rng,
    spi,
    spi::Spi,
    time::Hertz,
    Config,
    Peripherals,
};
use es_wifi_driver::{EsWifi as WifiDriver, EsWifiSocket as WifiSocket};

type WifiWake = Output<'static, PB13>;
type WifiReset = Output<'static, PE8>;
type WifiCs = Output<'static, PE0>;
type WifiReady = ExtiInput<'static, PE1>;

type WIFI_SPI = spi::Spi<'static, SPI3, DMA2_CH2, DMA2_CH1>;
type EsWifi = WifiDriver<WIFI_SPI, WifiCs, WifiReset, WifiWake, WifiReady>;
type EsWifiSocket<'d> = WifiSocket<'d, WIFI_SPI, WifiCs, WifiReset, WifiWake, WifiReady>;

pub struct Board {
    leds: [Option<Output<'static, AnyPin>>; 2],
    buttons: [Option<ExtiInput<'static, AnyPin>>; 1],
    wifi: Option<EsWifi>,
}

impl Device for Board {
    fn new() -> Self {
        let p = embassy_stm32::init(Default::default());
        let spi = spi::Spi::new(
            p.SPI3,
            p.PC10,
            p.PC12,
            p.PC11,
            p.DMA2_CH2,
            p.DMA2_CH1,
            Hertz(4_000_000),
            spi::Config::default(),
        );
        let _boot = Output::new(p.PB12, Level::Low, Speed::VeryHigh);
        let wake = Output::new(p.PB13, Level::Low, Speed::VeryHigh);
        let reset = Output::new(p.PE8, Level::Low, Speed::VeryHigh);
        let cs = Output::new(p.PE0, Level::High, Speed::VeryHigh);
        let ready = Input::new(p.PE1, Pull::Up);
        let ready = ExtiInput::new(ready, p.EXTI1);
        let wifi = WifiDriver::new(spi, cs, reset, wake, ready);

        Self {
            leds: [
                Some(Output::new(p.PA5.degrade(), Level::High, Speed::Low)),
                Some(Output::new(p.PB14.degrade(), Level::High, Speed::Low)),
            ],
            buttons: [Some(ExtiInput::new(
                Input::new(p.PC13.degrade(), Pull::Up),
                p.EXTI13.degrade(),
            ))],
            wifi: Some(wifi),
        }
    }

    type Led = Output<'static, AnyPin>;
    fn led(&mut self, n: usize) -> Option<Self::Led> {
        if n < self.leds.len() {
            self.leds[n].take()
        } else {
            None
        }
    }
    type Button = ExtiInput<'static, AnyPin>;
    fn button(&mut self, n: usize) -> Option<Self::Button> {
        if n < self.buttons.len() {
            self.buttons[n].take()
        } else {
            None
        }
    }
    type I2c1<'m> = I2c<'m, I2C1, DMA1_CH6, DMA1_CH7>;
    type Spi1<'m> = Spi<'m, SPI1, DMA1_CH3, DMA1_CH2>;

    type Tcp<'m> = EsWifi;
    fn tcp<'m>(&'m mut self) -> Option<Self::Tcp<'m>> {
        self.wifi.take()
    }
}
