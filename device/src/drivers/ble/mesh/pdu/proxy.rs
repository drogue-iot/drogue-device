use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use heapless::Vec;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Copy, Clone)]
pub enum SAR {
    Complete,
    First,
    Continuation,
    Last,
}

impl SAR {
    pub fn parse(data: u8) -> Result<Self, ParseError> {
        match data {
            0b00 => Ok(Self::Complete),
            0b01 => Ok(Self::First),
            0b10 => Ok(Self::Continuation),
            0b11 => Ok(Self::Last),
            _ => Err(ParseError::InvalidValue),
        }
    }
}

impl Into<u8> for SAR {
    fn into(self) -> u8 {
        match self {
            SAR::Complete => 0b00,
            SAR::First => 0b01,
            SAR::Continuation => 0b10,
            SAR::Last => 0b11,
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Copy, Clone)]
pub enum MessageType {
    NetworkPDU,
    MeshBeacon,
    ProxyConfiguration,
    ProvisioningPDU,
}

impl MessageType {
    pub fn parse(data: u8) -> Result<Self, ParseError> {
        match data {
            0x00 => Ok(Self::NetworkPDU),
            0x01 => Ok(Self::MeshBeacon),
            0x02 => Ok(Self::ProxyConfiguration),
            0x03 => Ok(Self::ProvisioningPDU),
            _ => Err(ParseError::InvalidValue),
        }
    }
}

impl Into<u8> for MessageType {
    fn into(self) -> u8 {
        match self {
            MessageType::NetworkPDU => 0x00,
            MessageType::MeshBeacon => 0x01,
            MessageType::ProxyConfiguration => 0x02,
            MessageType::ProvisioningPDU => 0x03,
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ProxyPDU {
    pub sar: SAR,
    pub message_type: MessageType,
    pub data: Vec<u8, 66>,
}

impl ProxyPDU {
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        let byte_one = ((self.sar as u8) << 6) | (self.message_type as u8);
        xmit.push(byte_one).map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.data)
            .map_err(|_| InsufficientBuffer)?;
        Ok(())
    }

    pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
        if data.len() < 1 {
            Err(ParseError::InvalidLength)?;
        }

        let sar = SAR::parse(data[0] >> 6)?;
        let message_type = MessageType::parse(data[0] & 0b00111111)?;
        let mut proxy_data = Vec::new();
        if data.len() > 1 {
            proxy_data.extend_from_slice(&data[1..])?;
        }

        Ok(Self {
            sar,
            message_type,
            data: proxy_data,
        })
    }
}
