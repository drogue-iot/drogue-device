use crate::drivers::ble::mesh::model::{
    FoundationIdentifier, HandlerError, Message, Model, ModelIdentifier, ReadableState, Sink, State,
};
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use crate::opcode;
use defmt::Format;
use heapless::Vec;

pub enum ConfigurationMessage {
    Beacon(BeaconMessage),
}

impl Message for ConfigurationMessage {
    fn opcode(&self) -> Opcode {
        match self {
            ConfigurationMessage::Beacon(inner) => inner.opcode(),
        }
    }

    fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        match self {
            ConfigurationMessage::Beacon(inner) => inner.emit(xmit),
        }
    }
}

pub struct ConfigurationServer;

impl Default for ConfigurationServer {
    fn default() -> Self {
        Self
    }
}

impl Model for ConfigurationServer {
    const IDENTIFIER: ModelIdentifier =
        ModelIdentifier::Foundation(FoundationIdentifier::Configuration);
    type MESSAGE = ConfigurationMessage;

    fn parse(&self, opcode: Opcode, parameters: &[u8]) -> Result<Option<Self::MESSAGE>, ParseError> {
        match opcode {
            CONFIG_BEACON_GET => Ok(Some(ConfigurationMessage::Beacon(
                BeaconMessage::parse_get(parameters)?,
            ))),
            CONFIG_BEACON_SET => Ok(None),
            _ => Ok(None),
        }
    }
}

opcode!( CONFIG_BEACON_GET 0x80, 0x09 );
opcode!( CONFIG_BEACON_SET 0x80, 0x0A );
opcode!( CONFIG_BEACON_STATUS 0x80, 0x0B );

#[derive(Format)]
pub enum BeaconMessage {
    Get,
    Set(bool),
    Status(bool),
}

impl Message for BeaconMessage {
    fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_BEACON_GET,
            Self::Set(_) => CONFIG_BEACON_SET,
            Self::Status(_) => CONFIG_BEACON_STATUS,
        }
    }

    fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        //self.opcode().emit(xmit)?;
        match self {
            BeaconMessage::Get => {}
            BeaconMessage::Set(val) => xmit
                .push(if *val { 1 } else { 0 })
                .map_err(|_| InsufficientBuffer)?,
            BeaconMessage::Status(val) => xmit
                .push(if *val { 1 } else { 0 })
                .map_err(|_| InsufficientBuffer)?,
        }
        Ok(())
    }
}

#[allow(unused)]
impl BeaconMessage {
    pub fn parse_get(parameters: &[u8]) -> Result<Self, ParseError> {
        defmt::info!("parse beacon get {:x}", parameters);
        if parameters.is_empty() {
            Ok(Self::Get)
        } else {
            Err(ParseError::InvalidLength)
        }
    }
}
