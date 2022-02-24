pub mod group_address;
pub mod unicast_address;
pub mod virtual_address;

pub use group_address::GroupAddress;
pub use unicast_address::UnicastAddress;
pub use virtual_address::{LabelUuid, VirtualAddress};

use crate::drivers::ble::mesh::pdu::ParseError;
use serde::{Deserialize, Serialize};

#[derive(Copy, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct InvalidAddress;

impl From<InvalidAddress> for ParseError {
    fn from(_: InvalidAddress) -> Self {
        ParseError::InvalidValue
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Address {
    Unassigned,
    Unicast(UnicastAddress),
    Virtual(VirtualAddress),
    Group(GroupAddress),
    LabelUuid(LabelUuid),
}

impl Address {
    pub fn as_bytes(&self) -> [u8; 2] {
        match self {
            Address::Unassigned => [0, 0],
            Address::Unicast(inner) => inner.as_bytes(),
            Address::Virtual(inner) => inner.as_bytes(),
            Address::Group(inner) => inner.as_bytes(),
            Address::LabelUuid(inner) => inner.virtual_address().as_bytes(),
        }
    }

    pub fn parse(data: [u8; 2]) -> Self {
        let val = u16::from_be_bytes(data);
        if data[0] == 0 && data[1] == 0 {
            Self::Unassigned
        } else if UnicastAddress::is_unicast_address(&data) {
            Self::Unicast(UnicastAddress(val))
        } else if GroupAddress::is_group_address(&data) {
            Self::Group(GroupAddress::parse_unchecked(data))
        } else {
            Self::Virtual(VirtualAddress(val))
        }
    }
}
