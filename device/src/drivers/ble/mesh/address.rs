use crate::drivers::ble::mesh::pdu::ParseError;
use core::convert::TryInto;
use core::ops::Add;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Address {
    Unassigned,
    Unicast(UnicastAddress),
    Virtual(VirtualAddress),
    Group(GroupAddress),
}

impl Address {
    pub fn as_bytes(&self) -> [u8; 2] {
        match self {
            Address::Unassigned => [0, 0],
            Address::Unicast(inner) => inner.as_bytes(),
            Address::Virtual(inner) => inner.as_bytes(),
            Address::Group(inner) => inner.as_bytes(),
        }
    }
}

impl Into<Address> for UnicastAddress {
    fn into(self) -> Address {
        Address::Unicast(self)
    }
}

impl Into<Address> for VirtualAddress {
    fn into(self) -> Address {
        Address::Virtual(self)
    }
}

impl Into<Address> for GroupAddress {
    fn into(self) -> Address {
        Address::Group(self)
    }
}

#[derive(Copy, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct InvalidAddress;

impl From<InvalidAddress> for ParseError {
    fn from(_: InvalidAddress) -> Self {
        ParseError::InvalidValue
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
pub struct UnicastAddress(u16);

#[cfg(feature = "defmt")]
impl defmt::Format for UnicastAddress {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "{=u16:04x}", self.0);
    }
}

impl UnicastAddress {
    pub fn as_bytes(&self) -> [u8; 2] {
        //[self.0[0], self.0[1]]
        self.0.to_be_bytes()
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

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct VirtualAddress(u16);

impl VirtualAddress {
    pub fn as_bytes(&self) -> [u8; 2] {
        self.0.to_be_bytes()
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GroupAddress(u16);

impl GroupAddress {
    pub fn as_bytes(&self) -> [u8; 2] {
        self.0.to_be_bytes()
    }
}

impl UnicastAddress {
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

impl VirtualAddress {
    pub fn is_virtual_address(data: &[u8; 2]) -> bool {
        data[0] & 0b11000000 == 0b10000000
    }

    pub fn parse(data: [u8; 2]) -> Result<Self, InvalidAddress> {
        if Self::is_virtual_address(&data) {
            Ok(VirtualAddress(u16::from_be_bytes(data)))
        } else {
            Err(InvalidAddress)
        }
    }
}

impl GroupAddress {
    pub fn is_group_address(data: &[u8; 2]) -> bool {
        data[0] & 0b11000000 == 0b11000000
    }

    pub fn parse(data: [u8; 2]) -> Result<Self, InvalidAddress> {
        if Self::is_group_address(&data) {
            Ok(GroupAddress(u16::from_be_bytes(data)))
        } else {
            Err(InvalidAddress)
        }
    }
}

impl Address {
    pub fn parse(data: [u8; 2]) -> Self {
        let val = u16::from_be_bytes(data);
        if data[0] == 0 && data[1] == 0 {
            Self::Unassigned
        } else if UnicastAddress::is_unicast_address(&data) {
            Self::Unicast(UnicastAddress(val))
        } else if GroupAddress::is_group_address(&data) {
            Self::Group(GroupAddress(val))
        } else {
            Self::Virtual(VirtualAddress(val))
        }
    }
}
