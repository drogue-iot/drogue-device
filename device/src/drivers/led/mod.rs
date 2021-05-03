use embedded_hal::digital::v2::{OutputPin, ToggleableOutputPin};

pub struct Led<P>
where
    P: OutputPin + ToggleableOutputPin,
{
    pin: P,
}

impl<P> Led<P>
where
    P: OutputPin + ToggleableOutputPin,
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
}

impl<P> From<P> for Led<P>
where
    P: OutputPin + ToggleableOutputPin,
{
    fn from(pin: P) -> Self {
        Self::new(pin)
    }
}
