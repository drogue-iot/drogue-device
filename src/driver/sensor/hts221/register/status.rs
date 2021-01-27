use crate::hal::i2c::I2cAddress;
use core::cell::Ref;
use core::ops::{DerefMut, Not};
use embedded_hal::blocking::i2c::WriteRead;

const STATUS: u8 = 0x27;

pub struct Status {
    temperature_available: bool,
    humidity_available: bool,
}

impl Status {
    pub fn read<I: DerefMut<Target = I2C>, I2C: WriteRead>(
        address: I2cAddress,
        i2c: &mut I,
    ) -> Status {
        let mut buf = [0; 1];
        let result = i2c.write_read(address.into(), &[STATUS], &mut buf);
        buf[0].into()
    }

    pub fn temperature_available(&self) -> bool {
        self.temperature_available
    }

    pub fn humidity_available(&self) -> bool {
        self.humidity_available
    }

    pub fn any_available(&self) -> bool {
        self.temperature_available || self.humidity_available
    }
}

impl Into<Status> for u8 {
    fn into(self) -> Status {
        Status {
            temperature_available: ((self & 0b01) != 0),
            humidity_available: ((self & 0b10) != 0),
        }
    }
}
