use crate::drivers::ble::mesh::address::{Address, InvalidAddress};
use core::convert::TryInto;
use core::ops::Add;
use core::ops::Sub;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
pub struct UnicastAddress(pub(crate) u16);

impl core::fmt::LowerHex for UnicastAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
        self.0.fmt(f)
    }
}

impl UnicastAddress {
    pub fn as_bytes(&self) -> [u8; 2] {
        self.0.to_be_bytes()
    }

    pub fn is_unicast_address(data: &[u8; 2]) -> bool {
        data[0] & 0b10000000 == 0
    }

    pub fn parse(data: [u8; 2]) -> Result<Self, InvalidAddress> {
        if Self::is_unicast_address(&data) {
            Ok(UnicastAddress(u16::from_be_bytes(data)))
        } else {
            Err(InvalidAddress)
        }
    }
}

impl Into<Address> for UnicastAddress {
    fn into(self) -> Address {
        Address::Unicast(self)
    }
}
#[cfg(feature = "defmt")]
impl defmt::Format for UnicastAddress {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "{=u16:04x}", self.0);
    }
}

impl From<UnicastAddress> for u16 {
    fn from(addr: UnicastAddress) -> Self {
        addr.0
    }
}

impl TryInto<UnicastAddress> for u16 {
    type Error = InvalidAddress;

    fn try_into(self) -> Result<UnicastAddress, Self::Error> {
        let bytes = self.to_be_bytes();
        UnicastAddress::parse([bytes[0], bytes[1]])
    }
}

impl Add<u8> for UnicastAddress {
    type Output = UnicastAddress;

    fn add(self, rhs: u8) -> Self::Output {
        Self(self.0 + rhs as u16)
    }
}

impl Sub<UnicastAddress> for UnicastAddress {
    type Output = u8;

    fn sub(self, rhs: UnicastAddress) -> Self::Output {
        (self.0 - rhs.0) as u8
    }
}
