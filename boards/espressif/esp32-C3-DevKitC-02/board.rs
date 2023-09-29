use embassy_esp32::{
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin, Pull},
    peripherals::{
        GPIO0, GPIO1, GPIO2, GPIO3, GPIO4, GPIO5, GPIO6, GPIO7, GPIO8, GPIO9, GPIO10, GPIO18,
        GPIO19, GPIO20, GPIO21, UARTE0, ADC1_CH0, ADC1_CH1, ADC1_CH2, ADC1_CH3, ADC1_CH4,
        ADC2_CH0,
    },
};

pub use embassy_esp32::{config::Config, wdt};

/// Red LED output
pub type RedLed = Output<'static, AnyPin>;

/// Input for user switch
pub type Switch = Input<'static, AnyPin>;

/// Board Support Package (BSP) type for the ESP32-C3-DevKitC-02
pub struct Esp32C3DevKitC02 {
    pub uarte0: UARTE0,
    pub red_led: RedLed,
    pub switch: Switch,
    pub gpio0: GPIO0,
    pub gpio1: GPIO1,
    pub gpio2: GPIO2,
    pub gpio3: GPIO3,
    pub gpio4: GPIO4,
    pub gpio5: GPIO5,
    pub gpio6: GPIO6,
    pub gpio7: GPIO7,
    pub gpio8: GPIO8,
    pub gpio9: GPIO9,
    pub gpio10: GPIO10,
    pub gpio18: GPIO18,
    pub gpio19: GPIO19,
    pub gpio20: GPIO20,
    pub gpio21: GPIO21,
}

impl Default for Esp32C3DevKitC02 {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl Esp32C3DevKitC02 {
    pub fn new(config: embassy_esp32::config::Config) -> Self {
        let p = embassy_esp32::init(config);
        Self {
            uarte0: p.UARTE0,
            red_led: Output::new(p.GPIO8.degrade(), Level::Low, OutputDrive::Standard),
            switch: Input::new(p.GPIO0.degrade(), Pull::Up),
            gpio0: p.GPIO0,
            gpio1: p.GPIO1,
            gpio2: p.GPIO2,
            gpio3: p.GPIO3,
            gpio4: p.GPIO4,
            gpio5: p.GPIO5,
            gpio6: p.GPIO6,
            gpio7: p.GPIO7,
            gpio8: p.GPIO8,
            gpio9: p.GPIO9,
            gpio10: p.GPIO10,
            gpio18: p.GPIO18,
            gpio19: p.GPIO19,
            gpio20: p.GPIO20,
            gpio21: p.GPIO21,
        }
    }
}

