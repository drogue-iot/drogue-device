use embedded_hal::digital::v2::OutputPin;

pub trait ActiveOutput {
    fn set_active<P: OutputPin>(pin: &mut P) -> Result<(), P::Error>;
    fn set_inactive<P: OutputPin>(pin: &mut P) -> Result<(), P::Error>;
}

pub struct ActiveHigh {}

pub struct ActiveLow {}

impl ActiveOutput for ActiveHigh {
    fn set_active<P: OutputPin>(pin: &mut P) -> Result<(), P::Error> {
        pin.set_high()
    }

    fn set_inactive<P: OutputPin>(pin: &mut P) -> Result<(), P::Error> {
        pin.set_low()
    }
}

impl ActiveOutput for ActiveLow {
    fn set_active<P: OutputPin>(pin: &mut P) -> Result<(), P::Error> {
        pin.set_low()
    }

    fn set_inactive<P: OutputPin>(pin: &mut P) -> Result<(), P::Error> {
        pin.set_high()
    }
}

pub trait InterruptPin {
    fn enable_interrupt(&mut self);
    fn check_interrupt(&mut self) -> bool;
    fn clear_interrupt(&mut self);
}
