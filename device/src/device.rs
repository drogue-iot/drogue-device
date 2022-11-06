use embedded_hal::digital::{ErrorType, InputPin, OutputPin};
use embedded_hal_async::{digital::Wait, i2c::I2c, spi::SpiBus};
use embedded_nal_async::TcpConnect;

/// A drogue device is a generic device that is implemented by any board. The capabilities
/// of different boards vary, therefore you might not get access to all functionality on
/// all boards.
pub trait Device {
    fn new() -> Self;

    /// Type representing LEDs for this device.
    type Led: OutputPin;
    /// Return the nth led, determined by the board mapping, if available.
    fn led(&mut self, _: usize) -> Option<Self::Led> {
        None
    }

    /// Type representing buttons for this device.
    type Button: InputPin + Wait;
    /// Return the nth button, determined by the board mapping, if available.
    fn button(&mut self, _: usize) -> Option<Self::Button> {
        None
    }

    type I2c1<'m>: I2c + 'm
    where
        Self: 'm;
    /// Return the first i2c peripheral, if available
    fn i2c1<'m>(&'m mut self) -> Option<Self::I2c1<'m>> {
        None
    }

    type Spi1<'m>: SpiBus + 'm
    where
        Self: 'm;
    /// Return the first spi peripheral, if available
    fn spi1<'m>(&'m mut self) -> Option<Self::Spi1<'m>> {
        None
    }

    type Tcp<'m>: TcpConnect + 'm
    where
        Self: 'm;
    /// Return access to TCP stack, if available
    fn tcp<'m>(&'m mut self) -> Option<Self::Tcp<'m>> {
        None
    }
}

impl ErrorType for DummyLed {
    type Error = ();
}

pub struct DummyLed;
impl OutputPin for DummyLed {
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct DummyButton;
