use crate::hal::i2c::I2cAddress;
use core::ops::DerefMut;
use embedded_hal::blocking::i2c::WriteRead;

// auto-increment variant of 2 bytes
const T_OUT: u8 = 0xAA;

pub struct Tout;

impl Tout {
    pub fn read<I: DerefMut<Target = I2C>, I2C: WriteRead>(
        address: I2cAddress,
        i2c: &mut I,
    ) -> i16 {
        let mut buf = [0; 2];
        let result = i2c.write_read(address.into(), &[T_OUT], &mut buf);
        i16::from_le_bytes(buf)
    }
}
