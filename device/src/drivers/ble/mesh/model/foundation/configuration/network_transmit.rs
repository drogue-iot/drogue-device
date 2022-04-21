use embassy::time::Duration;
use crate::drivers::ble::mesh::model::Message;
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use serde::{Deserialize, Serialize};
use crate::opcode;
use heapless::Vec;

opcode!( CONFIG_NETWORK_TRANSMIT_GET 0x80, 0x23);
opcode!( CONFIG_NETWORK_TRANSMIT_SET 0x80, 0x24);
opcode!( CONFIG_NETWORK_TRANSMIT_STATUS 0x80, 0x25);

#[derive(Copy, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct NetworkTransmitConfig {
    pub network_retransmit_count: u8,
    pub network_retransmit_interval_steps: u8,
}

impl Default for NetworkTransmitConfig {
    fn default() -> Self {
        Self {
            network_retransmit_count: 2,
            network_retransmit_interval_steps: 10,
        }
    }
}

impl NetworkTransmitConfig {
    pub fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() < 2 {
            Err(ParseError::InvalidLength)
        } else {
            let network_retransmit_count = parameters[0] & 0b11100000 >> 5;
            let network_retransmit_interval_steps = parameters[0] & 0b00011111;

            Ok(Self {
                network_retransmit_count,
                network_retransmit_interval_steps,
            })
        }
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        xmit.push(
            self.network_retransmit_count & 0b111 << 5
                | self.network_retransmit_interval_steps & 0b11111,
        )
        .map_err(|_| InsufficientBuffer)?;

        Ok(())
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum NetworkTransmitMessage {
    Get,
    Set(NetworkTransmitConfig),
    Status(NetworkTransmitConfig),
}

impl Message for NetworkTransmitMessage {
    fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_NETWORK_TRANSMIT_GET,
            Self::Set(_) => CONFIG_NETWORK_TRANSMIT_SET,
            Self::Status(_) => CONFIG_NETWORK_TRANSMIT_STATUS,
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        match self {
            Self::Get => {}
            Self::Set(inner) => inner.emit(xmit)?,
            Self::Status(inner) => inner.emit(xmit)?,
        }
        Ok(())
    }
}

#[allow(unused)]
impl NetworkTransmitMessage {
    pub fn parse_get(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.is_empty() {
            Ok(Self::Get)
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    pub fn parse_set(parameters: &[u8]) -> Result<Self, ParseError> {
        Ok(Self::Set(NetworkTransmitConfig::parse(parameters)?))
    }

    pub fn parse_status(parameters: &[u8]) -> Result<Self, ParseError> {
        Ok(Self::Set(NetworkTransmitConfig::parse(parameters)?))
    }
}
