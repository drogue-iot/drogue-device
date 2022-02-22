use crate::drivers::ble::mesh::composition::CompanyIdentifier;
use crate::drivers::ble::mesh::model::foundation::configuration::{
    CONFIGURATION_CLIENT, CONFIGURATION_SERVER,
};
use crate::drivers::ble::mesh::model::generic::{
    battery::{GENERIC_BATTERY_CLIENT, GENERIC_BATTERY_SERVER},
    onoff::{GENERIC_ONOFF_CLIENT, GENERIC_ONOFF_SERVER},
};
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use heapless::Vec;
use serde::{Deserialize, Serialize};

pub mod foundation;
pub mod generic;

#[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq)]
pub enum ModelIdentifier {
    SIG(u16),
    Vendor(CompanyIdentifier, u16),
}

#[cfg(feature = "defmt")]
impl defmt::Format for ModelIdentifier {
    fn format(&self, fmt: defmt::Formatter) {
        match *self {
            CONFIGURATION_SERVER => {
                defmt::write!(fmt, "Configuration Server (0x0000)");
            }
            CONFIGURATION_CLIENT => {
                defmt::write!(fmt, "Configuration Client (0x0001)");
            }
            GENERIC_ONOFF_SERVER => {
                defmt::write!(fmt, "Generic OnOff Server (0x1000)");
            }
            GENERIC_ONOFF_CLIENT => {
                defmt::write!(fmt, "Generic OnOff Client (0x1001)");
            }
            GENERIC_BATTERY_SERVER => {
                defmt::write!(fmt, "Generic Battery Server (0x100C)");
            }
            GENERIC_BATTERY_CLIENT => {
                defmt::write!(fmt, "Generic Battery Client (0x100D)");
            }
            ModelIdentifier::SIG(id) => match id {
                _ => {
                    defmt::write!(fmt, "SIG(0x{=u16:04x})", id);
                }
            },
            ModelIdentifier::Vendor(company_id, model_id) => {
                defmt::write!(fmt, "Vendor({}, 0x{=u16:04x})", company_id, model_id);
            }
        }
    }
}

impl ModelIdentifier {
    pub fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() == 2 {
            Ok(ModelIdentifier::SIG(u16::from_le_bytes([
                parameters[0],
                parameters[1],
            ])))
        } else if parameters.len() == 4 {
            Ok(ModelIdentifier::Vendor(
                CompanyIdentifier::parse(&parameters[0..=1])?,
                u16::from_le_bytes([parameters[2], parameters[3]]),
            ))
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        // NOTE: While so many things are big-endian... this is little-endian.
        // WHY OH WHY?
        match self {
            ModelIdentifier::SIG(model_id) => {
                xmit.extend_from_slice(&model_id.to_le_bytes())
                    .map_err(|_| InsufficientBuffer)?;
            }
            ModelIdentifier::Vendor(company_id, model_id) => {
                xmit.extend_from_slice(&company_id.0.to_le_bytes())
                    .map_err(|_| InsufficientBuffer)?;
                xmit.extend_from_slice(&model_id.to_le_bytes())
                    .map_err(|_| InsufficientBuffer)?;
            }
        }
        Ok(())
    }
}

#[cfg(feature = "defmt")]
pub trait Message: defmt::Format {
    fn opcode(&self) -> Opcode;
    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer>;
}

#[cfg(not(feature = "defmt"))]
pub trait Message {
    fn opcode(&self) -> Opcode;
    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer>;
}

pub enum HandlerError {
    Unhandled,
    NotConnected,
}

pub trait Model {
    const IDENTIFIER: ModelIdentifier;
    type Message: Message;

    fn parse(&self, opcode: Opcode, parameters: &[u8])
        -> Result<Option<Self::Message>, ParseError>;
}

#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Status {
    Success = 0x00,
    InvalidAddress = 0x01,
    InvalidModel = 0x02,
    InvalidAppKeyIndex = 0x03,
    InvalidNetKeyIndex = 0x04,
    InsufficientResources = 0x05,
    KeyIndexAlreadyStored = 0x06,
    InvalidPublishParameters = 0x07,
    NotASubscribeModel = 0x08,
    StorageFailure = 0x09,
    FeatureNotSupported = 0x0A,
    CannotUpdate = 0x0B,
    CannotRemove = 0x0C,
    CannotBind = 0x0D,
    TemporarilyUnableToChangeState = 0x0E,
    CannotSet = 0x0F,
    UnspecifiedError = 0x10,
    InvalidBinding = 0x11,
}
