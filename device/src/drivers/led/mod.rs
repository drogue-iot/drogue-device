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

    pub fn on(&mut self) -> Result<(), <P as OutputPin>::Error> {
        self.pin.set_high()
    }

    pub fn off(&mut self) -> Result<(), <P as OutputPin>::Error> {
        self.pin.set_low()
    }

    pub fn toggle(&mut self) -> Result<(), <P as ToggleableOutputPin>::Error> {
        self.pin.toggle()
    }

    pub fn state(&self) -> Result<bool, <P as OutputPin>::Error> {
        self.pin.is_set_high()
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
