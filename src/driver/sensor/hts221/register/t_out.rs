use crate::hal::i2c::I2cAddress;
use crate::prelude::Address;
use embedded_hal::blocking::i2c::WriteRead;
use crate::driver::i2c::I2cPeripheral;

// auto-increment variant of 2 bytes
const T_OUT: u8 = 0xAA;

pub struct Tout;

impl Tout {
    pub async fn read<I: WriteRead>(
        address: I2cAddress,
        i2c: Address<I2cPeripheral<I>>,
    ) -> Result<i16, I::Error> {
        unsafe {
            // # Safety
            // The call to `.write_read` is properly awaited for completion before allowing the buffer to drop.
            let mut buf = [0; 2];
            let result = i2c.write_read(address, &[T_OUT], &mut buf).await?;
            Ok(i16::from_le_bytes(buf))
        }
    }
}
