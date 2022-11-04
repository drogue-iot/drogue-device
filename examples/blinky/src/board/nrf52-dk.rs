use embassy_nrf::{
    gpio::{Input, Level, Output, OutputDrive, Pull},
    peripherals::{P0_11, P0_17},
};
impl crate::BlinkyBoard for super::Board {
    type Led = Output<'static, P0_17>;
    type Button = Input<'static, P0_11>;

    fn new() -> (Self::Led, Self::Button) {
        let p = embassy_nrf::init(Default::default());
        (
            Output::new(p.P0_17, Level::Low, OutputDrive::Standard),
            Input::new(p.P0_11, Pull::Up),
        )
    }
}
