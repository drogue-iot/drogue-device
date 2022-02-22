use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
use crate::drivers::ble::mesh::config::network::NetworkKeyHandle;
use crate::drivers::ble::mesh::pdu::lower::LowerPDU;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use heapless::Vec;

pub enum NetworkPDU {
    ObfuscatedAndEncrypted(ObfuscatedAndEncryptedNetworkPDU),
    Authenticated(CleartextNetworkPDU),
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum NetMic {
    Access([u8; 4]),
    Control([u8; 8]),
}

// todo: format vecs/arrays as hex
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ObfuscatedAndEncryptedNetworkPDU {
    pub(crate) ivi: u8, /* 1 bit */
    pub(crate) nid: u8, /* 7 bits */
    pub(crate) obfuscated: [u8; 6],
    pub(crate) encrypted_and_mic: Vec<u8, 28>,
}

impl ObfuscatedAndEncryptedNetworkPDU {
    pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
        let ivi_nid = data[0];
        let ivi = ivi_nid & 0b10000000 >> 7;
        let nid = ivi_nid & 0b01111111;
        let obfuscated = [data[1], data[2], data[3], data[4], data[5], data[6]];

        let encrypted_and_mic =
            Vec::from_slice(&data[7..]).map_err(|_| ParseError::InsufficientBuffer)?;

        Ok(Self {
            ivi,
            nid,
            obfuscated,
            encrypted_and_mic,
        })
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        let ivi_nid = ((self.ivi & 0b0000001) << 7) | (self.nid & 0b01111111);
        xmit.push(ivi_nid).map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.obfuscated)
            .map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.encrypted_and_mic)
            .map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CleartextNetworkPDU {
    pub(crate) network_key: NetworkKeyHandle,
    pub(crate) ivi: u8, /* 1 bit */
    pub(crate) nid: u8, /* 7 bits */
    // ctl: bool /* 1 bit */
    pub(crate) ttl: u8,  /* 7 bits */
    pub(crate) seq: u32, /* 24 bits */
    pub(crate) src: UnicastAddress,
    pub(crate) dst: Address,
    pub(crate) transport_pdu: LowerPDU,
}

impl CleartextNetworkPDU {}
