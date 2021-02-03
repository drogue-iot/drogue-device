use core::fmt::{Formatter, LowerHex, UpperHex};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct I2cAddress(u8);

impl I2cAddress {
    pub fn new(val: u8) -> Self {
        Self(val)
    }
}

impl Into<u8> for I2cAddress {
    fn into(self) -> u8 {
        self.0
    }
}

impl Into<I2cAddress> for u8 {
    fn into(self) -> I2cAddress {
        I2cAddress::new(self)
    }
}

impl LowerHex for I2cAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        LowerHex::fmt(&self.0, f)
    }
}

impl UpperHex for I2cAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        UpperHex::fmt(&self.0, f)
    }
}

pub struct I2c {

}






