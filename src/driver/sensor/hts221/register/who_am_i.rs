use crate::driver::i2c::I2cPeripheral;
use crate::hal::i2c::I2cAddress;
use crate::prelude::Address;
use embedded_hal::blocking::i2c::WriteRead;

const WHO_AM_I: u8 = 0x0F;

pub struct WhoAmI;

impl WhoAmI {
    pub async fn read<I: WriteRead>(
        address: I2cAddress,
        i2c: Address<I2cPeripheral<I>>,
    ) -> Result<I2cAddress, I::Error> {
        unsafe {
            // # Safety
            // The call to `.write_read` is properly awaited for completion before allowing the buffer to drop.
            let mut buf = [0; 1];
            let result = i2c.write_read(address, &[WHO_AM_I], &mut buf).await?;
            Ok(buf[0].into())
        }
    }
}
