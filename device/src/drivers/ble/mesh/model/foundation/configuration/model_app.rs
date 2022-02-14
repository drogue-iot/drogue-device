use crate::drivers::ble::mesh::address::UnicastAddress;
use crate::drivers::ble::mesh::model::foundation::configuration::{AppKeyIndex, KeyIndex};
use crate::drivers::ble::mesh::model::{Message, ModelIdentifier, Status};
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use crate::opcode;
use defmt::Format;
use heapless::Vec;

opcode!( CONFIG_MODEL_APP_BIND 0x80, 0x3D);
opcode!( CONFIG_MODEL_APP_STATUS 0x80, 0x3E);
opcode!( CONFIG_MODEL_APP_UNBIND 0x80, 0x3F);

#[derive(Format)]
pub enum ModelAppMessage {
    Bind(ModelAppPayload),
    Status(ModelAppStatusMessage),
    Unbind(ModelAppPayload),
}

impl ModelAppMessage {
    pub fn parse_bind(parameters: &[u8]) -> Result<Self, ParseError> {
        Ok(Self::Bind(ModelAppPayload::parse(parameters)?))
    }

    pub fn parse_unbind(parameters: &[u8]) -> Result<Self, ParseError> {
        Ok(Self::Unbind(ModelAppPayload::parse(parameters)?))
    }
}

impl Message for ModelAppMessage {
    fn opcode(&self) -> Opcode {
        match self {
            Self::Bind(_) => CONFIG_MODEL_APP_BIND,
            Self::Status(_) => CONFIG_MODEL_APP_STATUS,
            Self::Unbind(_) => CONFIG_MODEL_APP_UNBIND,
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        match self {
            ModelAppMessage::Bind(inner) => inner.emit_parameters(xmit),
            ModelAppMessage::Status(inner) => inner.emit_parameters(xmit),
            ModelAppMessage::Unbind(inner) => inner.emit_parameters(xmit),
        }
    }
}

#[derive(Format)]
pub struct ModelAppPayload {
    pub(crate) element_address: UnicastAddress,
    pub(crate) app_key_index: AppKeyIndex,
    pub(crate) model_identifier: ModelIdentifier,
}

impl ModelAppPayload {
    fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() >= 6 {
            // yes, swapped, because in *this* case it's little-endian
            let element_address = UnicastAddress::parse([parameters[1], parameters[0]])
                .map_err(|_| ParseError::InvalidValue)?;
            let app_key_index = AppKeyIndex(KeyIndex::parse_one(&parameters[2..=3])?);
            let model_identifier = ModelIdentifier::parse(&parameters[4..])?;
            Ok(Self {
                element_address,
                app_key_index,
                model_identifier,
            })
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        let addr_bytes = self.element_address.as_bytes();
        xmit.push(addr_bytes[1]).map_err(|_| InsufficientBuffer)?;
        xmit.push(addr_bytes[0]).map_err(|_| InsufficientBuffer)?;
        self.app_key_index.emit(xmit)?;
        self.model_identifier.emit(xmit)?;
        Ok(())
    }
}

#[derive(Format)]
pub struct ModelAppStatusMessage {
    pub(crate) status: Status,
    pub(crate) payload: ModelAppPayload,
}

impl ModelAppStatusMessage {
    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.push(self.status as u8)
            .map_err(|_| InsufficientBuffer)?;
        self.payload.emit_parameters(xmit)?;
        Ok(())
    }
}
