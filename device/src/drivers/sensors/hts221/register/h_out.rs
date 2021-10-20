use crate::traits::i2c::I2cAddress;
use embassy::traits::i2c::*;

// auto-increment variant of 2 bytes
const H_OUT: u8 = 0xA8;

pub struct Hout;

impl Hout {
    pub async fn read<I: I2c>(address: I2cAddress, i2c: &mut I) -> Result<i16, I::Error> {
        let mut buf = [0; 2];
        let _ = i2c.write_read(address.into(), &[H_OUT], &mut buf).await?;
        Ok(i16::from_le_bytes(buf))
    }
}
