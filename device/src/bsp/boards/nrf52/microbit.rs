use crate::bsp::Board;
use crate::drivers::led::{matrix::LedMatrix as LedMatrixDriver, ActiveHigh, Led};
use embassy_nrf::{
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin, Pull},
    gpiote::PortInput,
    peripherals::P0_14,
};

pub type LedMatrix = LedMatrixDriver<Output<'static, AnyPin>, 5, 5>;
pub type PinButtonA = Input<'static, P0_14>;
pub type ButtonA = PortInput<'static, P0_14>;

pub struct Microbit {
    pub led_matrix: LedMatrix,
    pub button_a: ButtonA,
}

impl Board for Microbit {
    type Peripherals = embassy_nrf::Peripherals;

    fn new(p: Self::Peripherals) -> Self {
        // LED Matrix
        let rows = [
            output_pin(p.P0_21.degrade()),
            output_pin(p.P0_22.degrade()),
            output_pin(p.P0_15.degrade()),
            output_pin(p.P0_24.degrade()),
            output_pin(p.P0_19.degrade()),
        ];

        let cols = [
            output_pin(p.P0_28.degrade()),
            output_pin(p.P0_11.degrade()),
            output_pin(p.P0_31.degrade()),
            output_pin(p.P1_05.degrade()),
            output_pin(p.P0_30.degrade()),
        ];

        Self {
            led_matrix: LedMatrixDriver::new(rows, cols),
            button_a: PortInput::new(Input::new(p.P0_14, Pull::Up)),
        }
    }
}

fn output_pin(pin: AnyPin) -> Output<'static, AnyPin> {
    Output::new(pin, Level::Low, OutputDrive::Standard)
}
