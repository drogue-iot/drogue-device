use embassy_stm32::{
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    peripherals::{PB14, PC13},
};
impl crate::BlinkyBoard for super::Board {
    type Led = Output<'static, PB14>;
    type Button = ExtiInput<'static, PC13>;

    fn new() -> (Self::Led, Self::Button) {
        let p = embassy_stm32::init(Default::default());
        (
            Output::new(p.PB14, Level::Low, Speed::VeryHigh),
            ExtiInput::new(Input::new(p.PC13, Pull::Up), p.EXTI13),
        )
    }
}
