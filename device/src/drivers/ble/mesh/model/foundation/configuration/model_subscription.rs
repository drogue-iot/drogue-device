use crate::drivers::ble::mesh::address::{GroupAddress, LabelUuid, UnicastAddress};
use crate::drivers::ble::mesh::model::{Message, ModelIdentifier, Status};
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use crate::opcode;
use heapless::Vec;
use serde::{Deserialize, Serialize};

opcode!( CONFIG_MODEL_SUBSCRIPTION_ADD 0x80, 0x1B);
opcode!( CONFIG_MODEL_SUBSCRIPTION_DELETE 0x80, 0x1C);
opcode!( CONFIG_MODEL_SUBSCRIPTION_DELETE_ALL 0x80, 0x1D);
opcode!( CONFIG_MODEL_SUBSCRIPTION_OVERWRITE 0x80, 0x1E);
opcode!( CONFIG_MODEL_SUBSCRIPTION_STATUS 0x80, 0x1F);
opcode!( CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_ADD 0x80, 0x20);
opcode!( CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_DELETE 0x80, 0x21);
opcode!( CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_OVERWRITE 0x80, 0x22);

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ModelSubscriptionMessage {
    Add(ModelSubscriptionAddMessage),
    Delete(ModelSubscriptionDeleteMessage),
    DeleteAll(ModelSubscriptionDeleteAllMessage),
    Overwrite(ModelSubscriptionOverwriteMessage),
    Status(ModelSubscriptionStatusMessage),
    VirtualAddressAdd(ModelSubscriptionAddMessage),
    VirtualAddressDelete(ModelSubscriptionDeleteMessage),
    VirtualAddressOverwrite(ModelSubscriptionOverwriteMessage),
}

#[allow(unused)]
impl Message for ModelSubscriptionMessage {
    fn opcode(&self) -> Opcode {
        match self {
            Self::Add(_) => CONFIG_MODEL_SUBSCRIPTION_ADD,
            Self::Delete(_) => CONFIG_MODEL_SUBSCRIPTION_DELETE,
            Self::DeleteAll(_) => CONFIG_MODEL_SUBSCRIPTION_DELETE_ALL,
            Self::Overwrite(_) => CONFIG_MODEL_SUBSCRIPTION_OVERWRITE,
            Self::Status(_) => CONFIG_MODEL_SUBSCRIPTION_STATUS,
            Self::VirtualAddressAdd(_) => CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_ADD,
            Self::VirtualAddressDelete(_) => CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_DELETE,
            Self::VirtualAddressOverwrite(_) => CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_OVERWRITE,
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        match self {
            ModelSubscriptionMessage::Add(inner) => inner.emit_parameters(xmit),
            ModelSubscriptionMessage::Delete(inner) => inner.emit_parameters(xmit),
            ModelSubscriptionMessage::DeleteAll(inner) => inner.emit_parameters(xmit),
            ModelSubscriptionMessage::Overwrite(inner) => inner.emit_parameters(xmit),
            ModelSubscriptionMessage::Status(inner) => inner.emit_parameters(xmit),
            ModelSubscriptionMessage::VirtualAddressAdd(inner) => inner.emit_parameters(xmit),
            ModelSubscriptionMessage::VirtualAddressDelete(inner) => inner.emit_parameters(xmit),
            ModelSubscriptionMessage::VirtualAddressOverwrite(inner) => inner.emit_parameters(xmit),
        }
    }
}

impl ModelSubscriptionMessage {
    pub fn parse_add(parameters: &[u8]) -> Result<Self, ParseError> {
        Ok(Self::Add(ModelSubscriptionAddMessage::parse(parameters)?))
    }

    pub fn parse_virtual_address_add(parameters: &[u8]) -> Result<Self, ParseError> {
        Ok(Self::Add(
            ModelSubscriptionAddMessage::parse_virtual_address(parameters)?,
        ))
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SubscriptionAddress {
    Unicast(UnicastAddress),
    Group(GroupAddress),
    Virtual(LabelUuid),
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ModelSubscriptionAddMessage {
    pub element_address: UnicastAddress,
    pub subscription_address: SubscriptionAddress,
    pub model_identifier: ModelIdentifier,
}

impl ModelSubscriptionAddMessage {
    pub fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() >= 6 {
            let element_address = UnicastAddress::parse([parameters[1], parameters[0]])?;
            let subscription_address = SubscriptionAddress::Unicast(UnicastAddress::parse([
                parameters[3],
                parameters[2],
            ])?);
            let model_identifier = ModelIdentifier::parse(&parameters[9..])?;
            Ok(Self {
                element_address,
                subscription_address,
                model_identifier,
            })
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    pub fn parse_virtual_address(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() >= 19 {
            let element_address = UnicastAddress::parse([parameters[1], parameters[0]])?;
            let subscription_address =
                SubscriptionAddress::Virtual(LabelUuid::parse(&parameters[2..=17])?);

            let model_identifier = ModelIdentifier::parse(&parameters[18..])?;
            Ok(Self {
                element_address,
                subscription_address,
                model_identifier,
            })
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    pub fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        todo!()
    }

    pub fn create_status_response(&self, status: Status) -> ModelSubscriptionStatusMessage {
        ModelSubscriptionStatusMessage {
            status,
            element_address: self.element_address,
            subscription_address: self.subscription_address,
            model_identifier: self.model_identifier,
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ModelSubscriptionDeleteMessage {}

impl ModelSubscriptionDeleteMessage {
    pub fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        todo!()
    }

    pub fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ModelSubscriptionDeleteAllMessage {}

impl ModelSubscriptionDeleteAllMessage {
    pub fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        todo!()
    }

    pub fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ModelSubscriptionOverwriteMessage {}

impl ModelSubscriptionOverwriteMessage {
    pub fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        todo!()
    }

    pub fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ModelSubscriptionStatusMessage {
    status: Status,
    element_address: UnicastAddress,
    subscription_address: SubscriptionAddress,
    model_identifier: ModelIdentifier,
}

impl ModelSubscriptionStatusMessage {
    pub fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        todo!("parse subscription status")
    }

    pub fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.push(self.status as u8)
            .map_err(|_| InsufficientBuffer)?;
        let addr_bytes = self.element_address.as_bytes();
        xmit.push(addr_bytes[1]).map_err(|_| InsufficientBuffer)?;
        xmit.push(addr_bytes[0]).map_err(|_| InsufficientBuffer)?;
        match self.subscription_address {
            SubscriptionAddress::Unicast(addr) => {
                let addr_bytes = addr.as_bytes();
                xmit.push(addr_bytes[1]).map_err(|_| InsufficientBuffer)?;
                xmit.push(addr_bytes[0]).map_err(|_| InsufficientBuffer)?;
            }
            SubscriptionAddress::Group(_addr) => {
                todo!("group address")
            }
            SubscriptionAddress::Virtual(addr) => {
                let addr_bytes = addr.virtual_address().as_bytes();
                xmit.push(addr_bytes[1]).map_err(|_| InsufficientBuffer)?;
                xmit.push(addr_bytes[0]).map_err(|_| InsufficientBuffer)?;
            }
        }
        self.model_identifier.emit(xmit)?;
        Ok(())
    }
}
