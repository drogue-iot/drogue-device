use embedded_hal::blocking::i2c::WriteRead;
use crate::hal::i2c::I2cAddress;
use core::cell::Ref;
use core::ops::DerefMut;

const WHO_AM_I: u8 = 0x0F;

pub struct WhoAmI;

impl WhoAmI {
    pub fn read<I: DerefMut<Target=I2C>, I2C: WriteRead>(address: I2cAddress, i2c: &mut I) -> I2cAddress {
        let mut buf = [0;1];
        let result = i2c.write_read( address.into(), &[WHO_AM_I], &mut buf);
        buf[0].into()
    }
}

