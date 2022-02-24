use crate::opcode;
use heapless::Vec;
use crate::drivers::ble::mesh::address::{Address, GroupAddress, UnicastAddress};
use crate::drivers::ble::mesh::address::virtual_address::LabelUuid;
use crate::drivers::ble::mesh::InsufficientBuffer;
use crate::drivers::ble::mesh::model::foundation::configuration::{AppKeyIndex, KeyIndex};
use crate::drivers::ble::mesh::model::{Message, ModelIdentifier, Status};
use crate::drivers::ble::mesh::model::foundation::configuration::model_publication::PublishAddress::Unicast;
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;

opcode!( CONFIG_MODEL_PUBLICATION_SET 0x03 );
opcode!( CONFIG_MODEL_PUBLICATION_GET 0x80, 0x18);
opcode!( CONFIG_MODEL_PUBLICATION_STATUS 0x80, 0x19);
opcode!( CONFIG_MODEL_PUBLICATION_VIRTUAL_ADDRESS_SET 0x80, 0x1A);

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ModelPublicationMessage {
    Get(ModelPublicationGetMessage),
    Set(ModelPublicationSetMessage),
    VirtualAddressSet(ModelPublicationSetMessage),
    Status(ModelPublicationStatusMessage),
}

impl Message for ModelPublicationMessage {
    fn opcode(&self) -> Opcode {
        match self {
            ModelPublicationMessage::Get(_) => CONFIG_MODEL_PUBLICATION_GET,
            ModelPublicationMessage::Set(_) => CONFIG_MODEL_PUBLICATION_SET,
            ModelPublicationMessage::VirtualAddressSet(_) => {
                CONFIG_MODEL_PUBLICATION_VIRTUAL_ADDRESS_SET
            }
            ModelPublicationMessage::Status(_) => CONFIG_MODEL_PUBLICATION_STATUS,
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        match self {
            ModelPublicationMessage::Get(inner) => inner.emit_parameters(xmit),
            ModelPublicationMessage::Set(inner) => inner.emit_parameters(xmit),
            ModelPublicationMessage::VirtualAddressSet(inner) => inner.emit_parameters(xmit),
            ModelPublicationMessage::Status(inner) => inner.emit_parameters(xmit),
        }
    }
}

impl ModelPublicationMessage {
    pub fn parse_set(parameters: &[u8]) -> Result<Self, ParseError> {
        Ok(Self::Set(ModelPublicationSetMessage::parse(parameters)?))
    }

    pub fn parse_virtual_address_set(parameters: &[u8]) -> Result<Self, ParseError> {
        Ok(Self::Set(
            ModelPublicationSetMessage::parse_virtual_address(parameters)?,
        ))
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ModelPublicationGetMessage {
    element_address: UnicastAddress,
    model_identifier: ModelIdentifier,
}

impl ModelPublicationGetMessage {
    fn emit_parameters<const N: usize>(
        &self,
        _xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PublishAddress {
    Unicast(UnicastAddress),
    Group(GroupAddress),
    Virtual(LabelUuid),
}

impl Into<Address> for PublishAddress {
    fn into(self) -> Address {
        match self {
            Unicast(inner) => Address::Unicast(inner),
            PublishAddress::Group(inner) => Address::Group(inner),
            PublishAddress::Virtual(inner) => Address::LabelUuid(inner),
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ModelPublicationSetMessage {
    pub element_address: UnicastAddress,
    pub publish_address: PublishAddress,
    pub app_key_index: AppKeyIndex,
    pub credential_flag: bool,
    pub publish_ttl: Option<u8>,
    pub publish_period: u8,
    pub publish_retransmit_count: u8,
    pub publish_retransmit_interval_steps: u8,
    pub model_identifier: ModelIdentifier,
}

impl ModelPublicationSetMessage {
    fn emit_parameters<const N: usize>(
        &self,
        _xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        todo!()
    }

    fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() >= 11 {
            let element_address = UnicastAddress::parse([parameters[1], parameters[0]])?;
            let publish_address =
                PublishAddress::Unicast(UnicastAddress::parse([parameters[3], parameters[2]])?);
            let app_key_index = AppKeyIndex(KeyIndex::parse_one(&parameters[4..=5])?);
            let credential_flag = (parameters[5] & 0b0001000) != 0;
            let publish_ttl = parameters[6];
            let publish_ttl = if publish_ttl == 0xFF {
                None
            } else {
                Some(publish_ttl)
            };
            let publish_period = parameters[7];
            let publish_retransmit_count = (parameters[8] & 0b11100000) >> 5;
            let publish_retransmit_interval_steps = parameters[8] & 0b00011111;
            let model_identifier = ModelIdentifier::parse(&parameters[9..])?;
            Ok(Self {
                element_address,
                publish_address,
                app_key_index,
                credential_flag,
                publish_ttl,
                publish_period,
                publish_retransmit_count,
                publish_retransmit_interval_steps,
                model_identifier,
            })
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    fn parse_virtual_address(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() >= 25 {
            let element_address = UnicastAddress::parse([parameters[1], parameters[0]])?;
            let publish_address = PublishAddress::Virtual(LabelUuid::parse(&parameters[2..=17])?);

            let app_key_index = AppKeyIndex(KeyIndex::parse_one(&parameters[18..=19])?);
            let credential_flag = (parameters[19] & 0b0001000) != 0;
            let publish_ttl = parameters[20];
            let publish_ttl = if publish_ttl == 0xFF {
                None
            } else {
                Some(publish_ttl)
            };
            let publish_period = parameters[21];
            let publish_retransmit_count = (parameters[22] & 0b11100000) >> 5;
            let publish_retransmit_interval_steps = parameters[22] & 0b00011111;
            let model_identifier = ModelIdentifier::parse(&parameters[23..])?;
            Ok(Self {
                element_address,
                publish_address,
                app_key_index,
                credential_flag,
                publish_ttl,
                publish_period,
                publish_retransmit_count,
                publish_retransmit_interval_steps,
                model_identifier,
            })
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    pub fn create_status_response(&self, status: Status) -> ModelPublicationStatusMessage {
        ModelPublicationStatusMessage {
            status,
            element_address: self.element_address,
            publish_address: self.publish_address,
            app_key_index: self.app_key_index,
            credential_flag: self.credential_flag,
            publish_ttl: self.publish_ttl,
            publish_period: self.publish_period,
            publish_retransmit_count: self.publish_retransmit_count,
            publish_retransmit_interval_steps: self.publish_retransmit_interval_steps,
            model_identifier: self.model_identifier,
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ModelPublicationStatusMessage {
    status: Status,
    element_address: UnicastAddress,
    publish_address: PublishAddress,
    app_key_index: AppKeyIndex,
    credential_flag: bool,
    publish_ttl: Option<u8>,
    publish_period: u8,
    publish_retransmit_count: u8,
    publish_retransmit_interval_steps: u8,
    model_identifier: ModelIdentifier,
}

impl ModelPublicationStatusMessage {
    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.push(self.status as u8)
            .map_err(|_| InsufficientBuffer)?;
        let addr_bytes = self.element_address.as_bytes();
        xmit.push(addr_bytes[1]).map_err(|_| InsufficientBuffer)?;
        xmit.push(addr_bytes[0]).map_err(|_| InsufficientBuffer)?;
        match self.publish_address {
            PublishAddress::Unicast(addr) => {
                let addr_bytes = addr.as_bytes();
                xmit.push(addr_bytes[1]).map_err(|_| InsufficientBuffer)?;
                xmit.push(addr_bytes[0]).map_err(|_| InsufficientBuffer)?;
            }
            PublishAddress::Group(_addr) => {
                todo!("group address")
            }
            PublishAddress::Virtual(addr) => {
                let addr_bytes = addr.virtual_address().as_bytes();
                xmit.push(addr_bytes[1]).map_err(|_| InsufficientBuffer)?;
                xmit.push(addr_bytes[0]).map_err(|_| InsufficientBuffer)?;
            }
        }
        self.app_key_index.emit(xmit)?;
        if self.credential_flag {
            if let Some(last) = xmit.last_mut() {
                *last = *last | 0b00001000;
            } else {
                return Err(InsufficientBuffer);
            }
        }
        if let Some(ttl) = self.publish_ttl {
            xmit.push(ttl).map_err(|_| InsufficientBuffer)?;
        } else {
            xmit.push(0xFF).map_err(|_| InsufficientBuffer)?;
        }
        xmit.push(self.publish_period)
            .map_err(|_| InsufficientBuffer)?;

        let retransmit = (self.publish_retransmit_count << 5)
            | (self.publish_retransmit_interval_steps & 0b00011111);
        xmit.push(retransmit).map_err(|_| InsufficientBuffer)?;
        self.model_identifier.emit(xmit)?;
        Ok(())
    }
}
