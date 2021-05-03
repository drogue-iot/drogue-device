/// Trait for a simple LED.
pub trait Led {
    type Error;

    fn set(&mut self, state: bool) -> Result<(), Self::Error>;
    fn state(&self) -> Result<bool, Self::Error>;

    fn on(&mut self) -> Result<(), Self::Error> {
        self.set(true)
    }

    fn off(&mut self) -> Result<(), Self::Error> {
        self.set(false)
    }

    fn toggle(&mut self) -> Result<(), Self::Error> {
        self.set(!self.state()?)
    }
}
