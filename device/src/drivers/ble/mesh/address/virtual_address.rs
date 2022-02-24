use crate::drivers::ble::mesh::address::{Address, InvalidAddress};
use crate::drivers::ble::mesh::crypto;
use crate::drivers::ble::mesh::pdu::ParseError;
use cmac::crypto_mac::InvalidKeyLength;
use core::convert::TryInto;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
pub struct VirtualAddress(pub(crate) u16);

impl VirtualAddress {
    pub fn as_bytes(&self) -> [u8; 2] {
        self.0.to_be_bytes()
    }

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

#[cfg(feature = "defmt")]
impl defmt::Format for VirtualAddress {
    fn format(&self, fmt: defmt::Formatter) {
        let bytes = self.as_bytes();
        defmt::write!(fmt, "{:x}{:x}", bytes[0], bytes[1])
    }
}

impl Into<Address> for VirtualAddress {
    fn into(self) -> Address {
        Address::Virtual(self)
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
pub struct LabelUuid {
    uuid: [u8; 16],
    address: VirtualAddress,
}

#[cfg(feature = "defmt")]
impl defmt::Format for LabelUuid {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "label={:x}{:x}{:x}{:x}{:x}{:x}{:x}{:x}{:x}{:x}{:x}{:x}{:x}{:x}{:x}{:x}; {}",
            self.uuid[0],
            self.uuid[1],
            self.uuid[2],
            self.uuid[3],
            self.uuid[4],
            self.uuid[5],
            self.uuid[6],
            self.uuid[7],
            self.uuid[8],
            self.uuid[9],
            self.uuid[10],
            self.uuid[11],
            self.uuid[12],
            self.uuid[13],
            self.uuid[14],
            self.uuid[15],
            self.address
        )
    }
}

impl LabelUuid {
    pub fn parse(uuid: &[u8]) -> Result<Self, ParseError> {
        if uuid.len() != 16 {
            Err(ParseError::InvalidLength)
        } else {
            Ok(
                Self::new(uuid.try_into().map_err(|_| ParseError::InvalidLength)?)
                    .map_err(|_| ParseError::InvalidLength)?,
            )
        }
    }

    pub fn new(uuid: [u8; 16]) -> Result<Self, InvalidKeyLength> {
        Ok(Self {
            uuid,
            address: Self::virtual_address_of(uuid)?,
        })
    }

    pub fn label_uuid(&self) -> &[u8] {
        &self.uuid
    }

    pub fn virtual_address(&self) -> VirtualAddress {
        self.address
    }

    pub fn virtual_address_of(uuid: [u8; 16]) -> Result<VirtualAddress, InvalidKeyLength> {
        let salt = crypto::s1(b"vtad")?;
        let hash = crypto::aes_cmac(&*salt.into_bytes(), &uuid)?;
        let hash = &mut hash.into_bytes()[14..=15];
        hash[0] = (0b00111111 & hash[0]) | 0b10000000;
        let hash = u16::from_be_bytes([hash[0], hash[1]]);
        Ok(VirtualAddress(hash))
    }
}
