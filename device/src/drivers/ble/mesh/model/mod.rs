use crate::drivers::ble::mesh::composition::CompanyIdentifier;
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use defmt::Format;
use heapless::Vec;

pub mod foundation;
pub mod generic;

#[derive(Copy, Clone, Eq, PartialEq, Format)]
pub enum FoundationIdentifier {
    Configuration,
    Health,
}

#[derive(Copy, Clone, Eq, PartialEq, Format)]
pub enum ModelIdentifier {
    Foundation(FoundationIdentifier),
    SIG(u16),
    Vendor(CompanyIdentifier, u16),
}

impl ModelIdentifier {
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        // NOTE: While so many things are big-endian... this is little-endian.
        // WHY OH WHY?
        match self {
            ModelIdentifier::Foundation(_) => { /* nope, don't do it */ }
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
    type MESSAGE: Message;

    fn parse(&self, opcode: Opcode, parameters: &[u8])
        -> Result<Option<Self::MESSAGE>, ParseError>;
}

#[derive(Copy, Clone, Format)]
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
