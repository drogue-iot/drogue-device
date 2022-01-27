use crate::traits::i2c::I2cAddress;
use embedded_hal_async::i2c::*;

// auto-increment variant of 2 bytes
const T_OUT: u8 = 0xAA;

pub struct Tout;

impl Tout {
    pub async fn read<I: I2c>(address: I2cAddress, i2c: &mut I) -> Result<i16, I::Error> {
        let mut buf = [0; 2];
        let _ = i2c.write_read(address.into(), &[T_OUT], &mut buf).await?;
        Ok(i16::from_le_bytes(buf))
    }
}
