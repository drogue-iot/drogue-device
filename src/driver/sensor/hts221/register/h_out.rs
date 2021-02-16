use crate::driver::i2c::I2cPeripheral;
use crate::api::i2c::I2cAddress;
use crate::prelude::Address;
use embedded_hal::blocking::i2c::WriteRead;

// auto-increment variant of 2 bytes
const H_OUT: u8 = 0xA8;

pub struct Hout;

impl Hout {
    pub async fn read<I: WriteRead>(
        address: I2cAddress,
        i2c: Address<I2cPeripheral<I>>,
    ) -> Result<i16, I::Error> {
        let mut buf = [0; 2];
        let result = i2c.write_read(address, &[H_OUT], &mut buf).await?;
        Ok(i16::from_le_bytes(buf))
    }
}
