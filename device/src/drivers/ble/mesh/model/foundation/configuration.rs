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

pub trait BeaconHandler {
    fn set(&mut self, val: bool);
    fn get(&self) -> bool;
}

pub trait ConfigurationServerHandler {
    type BEACON: BeaconHandler;
    fn beacon(&self) -> &Self::BEACON;
    fn beacon_mut(&mut self) -> &mut Self::BEACON;
}

pub struct ConfigurationServer<T: ConfigurationServerHandler> {
    sink: Option<Sink<ConfigurationMessage>>,
    handler: T,
}

impl<T: ConfigurationServerHandler> Model for ConfigurationServer<T> {
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

    fn connect(&mut self, sink: Sink<Self::MESSAGE>) {
        self.sink.replace(sink);
    }

    fn handle(&mut self, message: &Self::MESSAGE) -> Result<(), HandlerError> {
        match message {
            ConfigurationMessage::Beacon(BeaconMessage::Get) => {
                let response = self.handler.beacon().get();

                self.sink
                    .as_mut()
                    .ok_or(HandlerError::NotConnected)?
                    .transmit(ConfigurationMessage::Beacon(BeaconMessage::Status(
                        response,
                    )));
            }
            ConfigurationMessage::Beacon(BeaconMessage::Set(val)) => {
                self.handler.beacon_mut().set(*val);
            }
            _ => return Err(HandlerError::Unhandled),
        }
        Ok(())
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
