use core::marker::PhantomData;
use embedded_hal::digital::v2::{OutputPin, PinState};

pub mod matrix;

pub struct ActiveHigh;
pub struct ActiveLow;

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
        pin.set_state(if active {
            PinState::High
        } else {
            PinState::Low
        })
    }
}

impl<P> Active<P> for ActiveLow
where
    P: OutputPin,
{
    fn set(pin: &mut P, active: bool) -> Result<(), P::Error> {
        pin.set_state(if active {
            PinState::Low
        } else {
            PinState::High
        })
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
    pub fn new(pin: P) -> Self {
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
