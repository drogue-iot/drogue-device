use embedded_hal::digital::v2::OutputPin;
use core::marker::PhantomData;

pub mod exti_pin;

pub trait ActiveOutput {
    fn set_active<P: OutputPin>(pin: &mut P) -> Result<(), P::Error>;
    fn set_inactive<P: OutputPin>(pin: &mut P) -> Result<(), P::Error>;
}

pub struct ActiveHigh {
}

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
