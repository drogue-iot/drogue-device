use embedded_hal::digital::v2::{OutputPin, StatefulOutputPin, ToggleableOutputPin};

pub struct Led<P>
where
    P: StatefulOutputPin + ToggleableOutputPin,
{
    pin: P,
}

impl<P> Led<P>
where
    P: StatefulOutputPin + ToggleableOutputPin,
{
    pub fn new(pin: P) -> Self {
        Self { pin }
    }
}

pub enum LedError<P>
where
    P: StatefulOutputPin + ToggleableOutputPin,
{
    Stateful(<P as OutputPin>::Error),
    Toggleable(<P as ToggleableOutputPin>::Error),
}

impl<P> crate::traits::led::Led for Led<P>
where
    P: StatefulOutputPin + ToggleableOutputPin,
{
    type Error = LedError<P>;
    fn on(&mut self) -> Result<(), Self::Error> {
        self.pin.set_high().map_err(LedError::Stateful)
    }

    fn off(&mut self) -> Result<(), Self::Error> {
        self.pin.set_low().map_err(LedError::Stateful)
    }

    fn toggle(&mut self) -> Result<(), Self::Error> {
        self.pin.toggle().map_err(LedError::Toggleable)
    }

    fn state(&self) -> Result<bool, Self::Error> {
        self.pin.is_set_high().map_err(LedError::Stateful)
    }
}

impl<P> From<P> for Led<P>
where
    P: StatefulOutputPin + ToggleableOutputPin,
{
    fn from(pin: P) -> Self {
        Self::new(pin)
    }
}
