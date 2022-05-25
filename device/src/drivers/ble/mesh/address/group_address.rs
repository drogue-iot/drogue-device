use crate::drivers::ble::mesh::address::{Address, InvalidAddress};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GroupAddress {
    RFU(u16),
    AllProxies,
    AllFriends,
    AllRelays,
    AllNodes,
}

impl GroupAddress {
    pub fn as_bytes(&self) -> [u8; 2] {
        match self {
            GroupAddress::RFU(bytes) => bytes.to_be_bytes(),
            GroupAddress::AllProxies => [0xFF, 0xFC],
            GroupAddress::AllFriends => [0xFF, 0xFD],
            GroupAddress::AllRelays => [0xFF, 0xFE],
            GroupAddress::AllNodes => [0xFF, 0xFF],
        }
    }

    pub fn is_group_address(data: &[u8; 2]) -> bool {
        (data[0] & 0b11000000) == 0b11000000
    }

    pub fn parse(data: [u8; 2]) -> Result<Self, InvalidAddress> {
        if Self::is_group_address(&data) {
            Ok(Self::parse_unchecked(data))
        } else {
            Err(InvalidAddress)
        }
    }

    pub(crate) fn parse_unchecked(data: [u8; 2]) -> Self {
        match data {
            [0xFF, 0xFC] => Self::AllProxies,
            [0xFF, 0xFD] => Self::AllFriends,
            [0xFF, 0xFE] => Self::AllRelays,
            [0xFF, 0xFF] => Self::AllNodes,
            _ => Self::RFU(u16::from_be_bytes(data)),
        }
    }
}

impl Into<Address> for GroupAddress {
    fn into(self) -> Address {
        Address::Group(self)
    }
}
