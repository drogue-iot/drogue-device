use embassy_nrf::{
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin, Pull},
    peripherals::{
        NVMC, P0_02, P0_03, P0_04, P0_05, P0_06, P0_07, P0_08, P0_09, P0_10, P0_11, P0_12, P0_13,
        P0_14, P0_15, P0_16, P0_24, P0_25, P0_26, P0_27, P0_28, P0_29, P0_30, P0_31, P1_02, P1_08,
        P1_09, P1_10, P1_15, PWM0, UARTE0, USBD, WDT,
    },
};

/// Pin for red LED
pub type RedLedPin = P1_15;

/// Red LED output
pub type RedLed = Output<'static, AnyPin>;

/// Pin for blue LED
pub type BlueLedPin = P1_10;

/// Blue LED output
pub type BlueLed = Output<'static, AnyPin>;

/// Pin for user switch
pub type PinSwitch = P1_02;

/// Input for user switch
pub type Switch = Input<'static, AnyPin>;

#[cfg(feature = "express")]
mod express {
    use embassy_nrf::{
        interrupt,
        peripherals::{P0_17, P0_19, P0_20, P0_21, P0_22, P0_23, QSPI},
        qspi,
    };

    pub const EXTERNAL_FLASH_SIZE: usize = 2097152;
    pub const EXTERNAL_FLASH_BLOCK_SIZE: usize = 256;
    pub type ExternalFlash<'d> = qspi::Qspi<'d, QSPI, EXTERNAL_FLASH_SIZE>;

    /// Pins for External QSPI flash
    pub struct ExternalFlashPins {
        pub qspi: QSPI,
        pub sck: P0_19,
        pub csn: P0_20,
        pub io0: P0_17,
        pub io1: P0_22,
        pub io2: P0_23,
        pub io3: P0_21,
    }

    impl ExternalFlashPins {
        /// Configure an external flash instance based on pins
        pub fn configure<'d>(self) -> ExternalFlash<'d> {
            let mut config = qspi::Config::default();
            config.read_opcode = qspi::ReadOpcode::READ4IO;
            config.write_opcode = qspi::WriteOpcode::PP4O;
            config.write_page_size = qspi::WritePageSize::_256BYTES;
            let irq = interrupt::take!(QSPI);
            let mut q: qspi::Qspi<'_, _, EXTERNAL_FLASH_SIZE> = qspi::Qspi::new(
                self.qspi, irq, self.sck, self.csn, self.io0, self.io1, self.io2, self.io3, config,
            );

            // Setup QSPI
            let mut status = [4; 2];
            q.blocking_custom_instruction(0x05, &[], &mut status[..1])
                .unwrap();

            q.blocking_custom_instruction(0x35, &[], &mut status[1..2])
                .unwrap();

            if status[1] & 0x02 == 0 {
                status[1] |= 0x02;
                q.blocking_custom_instruction(0x01, &status, &mut [])
                    .unwrap();
            }
            q
        }
    }
}

#[cfg(feature = "express")]
pub use express::*;

/// Board Support Package (BSP) type for the Adafruit Feather nRF52
pub struct AdafruitFeatherNrf52 {
    pub uarte0: UARTE0,
    pub pwm0: PWM0,
    pub usbd: USBD,
    pub nvmc: NVMC,
    pub wdt: WDT,
    pub red_led: RedLed,
    pub blue_led: BlueLed,
    pub switch: Switch,
    pub d2: P0_10,
    pub tx: P0_25,
    pub rx: P0_24,
    pub miso: P0_15,
    pub mosi: P0_13,
    pub sck: P0_14,
    pub a5: P0_03,
    pub a4: P0_02,
    pub a3: P0_28,
    pub a2: P0_30,
    pub a1: P0_05,
    pub a0: P0_04,
    pub aref: P0_31,
    pub sda: P0_12,
    pub scl: P0_11,
    pub d5: P1_08,
    pub d6: P0_07,
    pub d9: P0_26,
    pub d10: P0_27,
    pub d11: P0_06,
    pub d12: P0_08,
    pub d13: P1_09,
    #[cfg(feature = "express")]
    pub external_flash: ExternalFlashPins,
    pub neopixel: P0_16,
    pub nfc1: P0_09,
    pub voltage: P0_29,
}

impl Default for AdafruitFeatherNrf52 {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl AdafruitFeatherNrf52 {
    pub fn new(config: embassy_nrf::config::Config) -> Self {
        let p = embassy_nrf::init(config);
        Self {
            uarte0: p.UARTE0,
            pwm0: p.PWM0,
            usbd: p.USBD,
            nvmc: p.NVMC,
            wdt: p.WDT,
            red_led: Output::new(p.P1_15.degrade(), Level::Low, OutputDrive::Standard),
            blue_led: Output::new(p.P1_10.degrade(), Level::Low, OutputDrive::Standard),
            switch: Input::new(p.P1_02.degrade(), Pull::Up),

            d2: p.P0_10,
            tx: p.P0_25,
            rx: p.P0_24,
            miso: p.P0_15,
            mosi: p.P0_13,
            sck: p.P0_14,
            a5: p.P0_03,
            a4: p.P0_02,
            a3: p.P0_28,
            a2: p.P0_30,
            a1: p.P0_05,
            a0: p.P0_04,
            aref: p.P0_31,
            sda: p.P0_12,
            scl: p.P0_11,
            d5: p.P1_08,
            d6: p.P0_07,
            d9: p.P0_26,
            d10: p.P0_27,
            d11: p.P0_06,
            d12: p.P0_08,
            d13: p.P1_09,
            #[cfg(feature = "express")]
            external_flash: ExternalFlashPins {
                qspi: p.QSPI,
                sck: p.P0_19,
                csn: p.P0_20,
                io0: p.P0_17,
                io1: p.P0_22,
                io2: p.P0_23,
                io3: p.P0_21,
            },
            neopixel: p.P0_16,
            nfc1: p.P0_09,
            voltage: p.P0_29,
        }
    }
}
