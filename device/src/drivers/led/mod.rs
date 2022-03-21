use core::marker::PhantomData;
use embedded_hal::digital::v2::{OutputPin, PinState};

pub use crate::drivers::ActiveHigh;
pub use crate::drivers::ActiveLow;

pub mod matrix;

#[cfg(all(feature = "neopixel", feature = "nrf"))]
pub mod neopixel;

pub trait Active<P>
where
    P: OutputPin,
{
    fn set(pin: &mut P, active: bool) -> Result<(), P::Error>;
}

impl<P> Active<P> for ActiveHigh
where
    P: OutputPin,
{
    fn set(pin: &mut P, active: bool) -> Result<(), P::Error> {
        pin.set_state(PinState::from(active))
    }
}

impl<P> Active<P> for ActiveLow
where
    P: OutputPin,
{
    fn set(pin: &mut P, active: bool) -> Result<(), P::Error> {
        pin.set_state(!PinState::from(active))
    }
}

pub struct Led<P, ACTIVE = ActiveHigh>
where
    P: OutputPin,
    ACTIVE: Active<P>,
{
    pin: P,
    _active: PhantomData<ACTIVE>,
}

impl<P, ACTIVE> Led<P, ACTIVE>
where
    P: OutputPin,
    ACTIVE: Active<P>,
{
    pub fn new(mut pin: P) -> Self {
        ACTIVE::set(&mut pin, false).ok();
        Self {
            pin,
            _active: PhantomData,
        }
    }
}

impl<P, ACTIVE> crate::traits::led::Led for Led<P, ACTIVE>
where
    P: OutputPin,
    ACTIVE: Active<P>,
{
    type Error = P::Error;

    fn on(&mut self) -> Result<(), Self::Error> {
        ACTIVE::set(&mut self.pin, true)
    }

    fn off(&mut self) -> Result<(), Self::Error> {
        ACTIVE::set(&mut self.pin, false)
    }
}

impl<P> From<P> for Led<P>
where
    P: OutputPin,
{
    fn from(pin: P) -> Self {
        Self::new(pin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_active_high() {
        let mut pin = TestOutputPin::new();
        ActiveHigh::set(&mut pin, true).unwrap();
        assert!(pin.state == PinState::High);

        ActiveHigh::set(&mut pin, false).unwrap();
        assert!(pin.state == PinState::Low);
    }

    #[test]
    fn test_active_low() {
        let mut pin = TestOutputPin::new();
        ActiveLow::set(&mut pin, true).unwrap();
        assert!(pin.state == PinState::Low);

        ActiveLow::set(&mut pin, false).unwrap();
        assert!(pin.state == PinState::High);
    }

    struct TestOutputPin {
        state: PinState,
    }

    impl OutputPin for TestOutputPin {
        type Error = ();
        fn set_high(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
        fn set_low(&mut self) -> Result<(), Self::Error> {
            Ok(())
        }
        fn set_state(&mut self, s: PinState) -> Result<(), Self::Error> {
            self.state = s;
            Ok(())
        }
    }

    impl TestOutputPin {
        fn new() -> Self {
            Self {
                state: PinState::High,
            }
        }
    }
}
