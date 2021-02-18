pub mod tcp;

use core::fmt::{Debug, Formatter};

#[derive(Debug)]
pub enum IpAddress {
    V4(IpAddressV4),
}

pub struct IpAddressV4(u8, u8, u8, u8);

impl IpAddressV4 {
    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        IpAddressV4(a, b, c, d)
    }
}

impl Debug for IpAddressV4 {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0, self.1, self.2, self.3)
    }
}
