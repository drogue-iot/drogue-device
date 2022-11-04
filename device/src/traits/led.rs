use core::convert::Infallible;
use embedded_hal::digital::OutputPin;

pub trait Led {
    type Error;
    fn on(&mut self) -> Result<(), Self::Error>;
    fn off(&mut self) -> Result<(), Self::Error>;
}

impl<P> Led for P
where
    P: OutputPin,
{
    type Error = Infallible;
    fn on(&mut self) -> Result<(), Self::Error> {
        self.set_high().ok();
        Ok(())
    }

    fn off(&mut self) -> Result<(), Self::Error> {
        self.set_low().ok();
        Ok(())
    }
}
