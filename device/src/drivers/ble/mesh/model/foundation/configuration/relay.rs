use crate::drivers::ble::mesh::model::Message;
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use crate::opcode;
use heapless::Vec;
use serde::{Deserialize, Serialize};

opcode!( CONFIG_RELAY_GET 0x80, 0x26);
opcode!( CONFIG_RELAY_SET 0x80, 0x27);
opcode!( CONFIG_RELAY_STATUS 0x80, 0x28);

#[derive(Copy, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Relay {
    SupportedDisabled = 0x00,
    SupportedEnabled = 0x01,
    NotSupported = 0x02,
}

impl Relay {
    pub fn parse(data: u8) -> Result<Self, ParseError> {
        match data {
            0x00 => Ok(Self::SupportedDisabled),
            0x01 => Ok(Self::SupportedEnabled),
            0x02 => Ok(Self::NotSupported),
            _ => Err(ParseError::InvalidValue),
        }
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        xmit.push(*self as u8).map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}

#[derive(Copy, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct RelayConfig {
    pub relay: Relay,
    pub relay_retransmit_count: u8,
    pub relay_retransmit_interval_steps: u8,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            relay: Relay::SupportedEnabled,
            relay_retransmit_count: 1,
            relay_retransmit_interval_steps: 20,
        }
    }
}

impl RelayConfig {
    pub fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() < 2 {
            Err(ParseError::InvalidLength)
        } else {
            let relay = Relay::parse(parameters[0])?;
            let relay_retransmit_count = parameters[0] & 0b11100000 >> 5;
            let relay_retransmit_interval_steps = parameters[0] & 0b00011111;

            Ok(Self {
                relay,
                relay_retransmit_count,
                relay_retransmit_interval_steps,
            })
        }
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        self.relay.emit(xmit)?;

        xmit.push(
            self.relay_retransmit_count & 0b111 << 5
                | self.relay_retransmit_interval_steps & 0b11111,
        )
        .map_err(|_| InsufficientBuffer)?;

        Ok(())
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum RelayMessage {
    Get,
    Set(RelayConfig),
    Status(RelayConfig),
}

impl Message for RelayMessage {
    fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_RELAY_GET,
            Self::Set(_) => CONFIG_RELAY_SET,
            Self::Status(_) => CONFIG_RELAY_STATUS,
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
impl RelayMessage {
    pub fn parse_get(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.is_empty() {
            Ok(Self::Get)
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    pub fn parse_set(parameters: &[u8]) -> Result<Self, ParseError> {
        Ok(Self::Set(RelayConfig::parse(parameters)?))
    }

    pub fn parse_status(parameters: &[u8]) -> Result<Self, ParseError> {
        Ok(Self::Set(RelayConfig::parse(parameters)?))
    }
}
