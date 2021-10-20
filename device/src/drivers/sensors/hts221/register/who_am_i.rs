use crate::traits::i2c::I2cAddress;
use embassy::traits::i2c::*;

const WHO_AM_I: u8 = 0x0F;

pub struct WhoAmI;

impl WhoAmI {
    pub async fn read<I: I2c>(address: I2cAddress, i2c: &mut I) -> Result<I2cAddress, I::Error> {
        let mut buf = [0; 1];
        let _ = i2c
            .write_read(address.into(), &[WHO_AM_I], &mut buf)
            .await?;
        Ok(buf[0].into())
    }
}
