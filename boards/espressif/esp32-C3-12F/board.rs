//! Board Support Package (BSP) for ESP32C3-12F

use embassy_esp32::{
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin, Pull},
    peripherals::{
        ADC1, GPIO0, GPIO1, GPIO2, GPIO3, GPIO4, GPIO5, GPIO6, GPIO7, GPIO8, GPIO9, GPIO10,
        GPIO18, GPIO19, GPIO20, GPIO21, PWM0, RNG, SAADC, TIMER0, TWISPI0, UARTE0,
    },
};

/// Button for Reset
pub type ResetButton = Input<'static, AnyPin>;

/// Button for Program
pub type ProgramButton = Input<'static, AnyPin>;

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

/// Button for Reset
pub type ResetButton = Input<'static, AnyPin>;

/// Button for Program
pub type ProgramButton = Input<'static, AnyPin>;

/// Represents all the peripherals and pins available for the ESP32-C3-12F.
pub struct Esp32C3Board {
    /// UART0 peripheral
    pub uarte0: UARTE0,
    /// TIMER0 peripheral
    pub timer0: TIMER0,
    /// PWM0 peripheral
    pub pwm0: PWM0,
    /// Random number generator
    pub rng: RNG,
    /// Analog digital converter
    pub saadc: SAADC,
    /// SPI/I2C peripheral
    pub twispi0: TWISPI0,
    /// Reset button
    pub reset_button: ResetButton,
    /// Program button
    pub program_button: ProgramButton,
    // Add other GPIOs, ADCs, etc. here
}

impl Default for Esp32C3Board {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl Esp32C3Board {
    /// Create a new instance based on HAL configuration
    pub fn new(config: embassy_esp32::config::Config) -> Self {
        let p = embassy_esp32::init(config);

        Self {
            uarte0: p.UARTE0,
            timer0: p.TIMER0,
            pwm0: p.PWM0,
            rng: p.RNG,
            saadc: p.SAADC,
            twispi0: p.TWISPI0,
            reset_button: Input::new(p.GPIO9.degrade(), Pull::Up),
            program_button: Input::new(p.GPIO8.degrade(), Pull::Up),
            // Initialize other GPIOs, ADCs, etc. here
        }
    }
}
