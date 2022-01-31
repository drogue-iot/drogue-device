use core::convert::TryInto;
use defmt::Format;

#[derive(Copy, Clone, Format, PartialEq)]
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

#[derive(Copy, Clone, Format, PartialEq)]
pub struct InvalidAddress;

#[derive(Copy, Clone, Format, PartialEq)]
pub struct UnicastAddress([u8; 2]);

impl UnicastAddress {
    pub fn as_bytes(&self) -> [u8; 2] {
        [self.0[0], self.0[1]]
    }
}

impl TryInto<UnicastAddress> for u16 {
    type Error = InvalidAddress;

    fn try_into(self) -> Result<UnicastAddress, Self::Error> {
        let bytes = self.to_be_bytes();
        UnicastAddress::parse([bytes[0], bytes[1]])
    }
}

#[derive(Copy, Clone, Format, PartialEq)]
pub struct VirtualAddress([u8; 2]);

impl VirtualAddress {
    pub fn as_bytes(&self) -> [u8; 2] {
        [self.0[0], self.0[1]]
    }
}

#[derive(Copy, Clone, Format, PartialEq)]
pub struct GroupAddress([u8; 2]);

impl GroupAddress {
    pub fn as_bytes(&self) -> [u8; 2] {
        [self.0[0], self.0[1]]
    }
}

impl UnicastAddress {
    pub fn is_unicast_address(data: &[u8; 2]) -> bool {
        data[0] & 0b10000000 == 0
    }

    pub fn parse(data: [u8; 2]) -> Result<Self, InvalidAddress> {
        if Self::is_unicast_address(&data) {
            Ok(UnicastAddress(data))
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
            Ok(VirtualAddress(data))
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
            Ok(GroupAddress(data))
        } else {
            Err(InvalidAddress)
        }
    }
}

impl Address {
    pub fn parse(data: [u8; 2]) -> Self {
        if data[0] == 0 && data[1] == 0 {
            Self::Unassigned
        } else if UnicastAddress::is_unicast_address(&data) {
            Self::Unicast(UnicastAddress([data[0], data[1]]))
        } else if GroupAddress::is_group_address(&data) {
            Self::Group(GroupAddress([data[0], data[1]]))
        } else {
            Self::Virtual(VirtualAddress([data[0], data[1]]))
        }
    }
}
