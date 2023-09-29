//! Board Support Package (BSP) for ESP32C3-12F

use embassy_esp32::gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin, Pull};
use embassy_esp32::peripherals::{
    ADC1, GPIO0, GPIO1, GPIO10, GPIO18, GPIO19, GPIO2, GPIO20, GPIO21, GPIO3, GPIO4, GPIO5, GPIO6,
    GPIO7, GPIO8, GPIO9, PWM, SPI1, SPI2, SPI3, UART1, UART2,
};

/// ADC1 peripheral
pub type Adc = ADC1;

/// GPIO1 with ADC1_1 functionality
pub type PinGpio1 = GPIO1;

/// GPIO2 with ADC1_2 and MISO functionality
pub type PinGpio2 = GPIO2;

/// GPIO3 with ADC1_3 functionality
pub type PinGpio3 = GPIO3;

/// GPIO4 with ADC1_4, FSPIHD, MTMS functionality
pub type PinGpio4 = GPIO4;

/// GPIO5 with ADC2_0, FSPIWP, MTDI functionality
pub type PinGpio5 = GPIO5;

/// GPIO0 with ADC1_0, XTAL_32K_P functionality
pub type PinGpio0 = GPIO0;

/// GPIO19 with D+ functionality
pub type PinGpio19 = GPIO19;

/// GPIO18 with D- functionality
pub type PinGpio18 = GPIO18;

/// GPIO10 with FSPICSO functionality
pub type PinGpio10 = GPIO10;

/// GPIO9 with DTR, BOOT functionality
pub type PinGpio9 = GPIO9;

/// GPIO8
pub type PinGpio8 = GPIO8;

/// GPIO6 with FSPICLK, MTCK, SCLK functionality
pub type PinGpio6 = GPIO6;

/// GPIO7 with FSPID, MTDO, MOSI functionality
pub type PinGpio7 = GPIO7;

/// GPIO21 with U0TXD, TX functionality
pub type PinGpio21 = GPIO21;

/// GPIO20 with U0RXD, RX functionality
pub type PinGpio20 = GPIO20;

/// Reset button
pub type ResetButton = Input<'static, AnyPin>;

/// Program button
pub type ProgramButton = Input<'static, AnyPin>;

/// Board Support Package (BSP) type for ESP32C3-12F
pub struct Esp32C3Board {
    pub adc: Adc,
    pub pwm: PWM,
    pub spi1: SPI1,
    pub spi2: SPI2,
    pub spi3: SPI3,
    pub uart1: UART1,
    pub uart2: UART2,
    pub reset_button: ResetButton,
    pub program_button: ProgramButton,
    // Add other peripherals and pins here
}

impl Default for Esp32C3Board {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl Esp32C3Board {
    pub fn new() -> Self {
        let p = embassy_esp32::init(Default::default());

        Self {
            adc: p.ADC1,
            pwm: p.PWM,
            spi1: p.SPI1,
            spi2: p.SPI2,
            spi3: p.SPI3,
            uart1: p.UART1,
            uart2: p.UART2,
            reset_button: Input::new(p.GPIO9.degrade(), Pull::Up), // Assuming GPIO9 for Reset button
            program_button: Input::new(p.GPIO10.degrade(), Pull::Up), // Assuming GPIO10 for Program button
            // Initialize other peripherals and pins here
        }
    }
}

