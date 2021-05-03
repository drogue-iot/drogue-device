use crate::traits::led::Led;
use embedded_hal::digital::v2::OutputPin;

pub struct GpioLed<P>
where
    P: OutputPin,
{
    pin: P,
    state: bool,
}

impl<P> GpioLed<P>
where
    P: OutputPin,
{
    pub fn new(pin: P) -> Self {
        Self { pin, state: false }
    }
}

impl<P> Led for GpioLed<P>
where
    P: OutputPin,
{
    type Error = P::Error;

    fn set(&mut self, state: bool) -> Result<(), Self::Error> {
        match state {
            true => self.pin.set_high(),
            false => self.pin.set_low(),
        }?;
        self.state = state;
        Ok(())
    }

    fn state(&self) -> Result<bool, Self::Error> {
        Ok(self.state)
    }
}

impl<P> From<P> for GpioLed<P>
where
    P: OutputPin,
{
    fn from(pin: P) -> Self {
        Self::new(pin)
    }
}
