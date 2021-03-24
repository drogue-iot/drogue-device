use crate::api::spi::SpiError;
use stm32l0xx_hal::spi::Error;

impl Into<SpiError> for Error {
    fn into(self) -> SpiError {
        match self {
            Error::Overrun => SpiError::Overrun,
            Error::ModeFault => SpiError::ModeFault,
            Error::Crc => SpiError::Crc,
            _ => SpiError::Unknown,
        }
    }
}
