use crate::drivers::ble::mesh::model::foundation::configuration::app_key::{
    AppKeyMessage, CONFIG_APPKEY_ADD, CONFIG_APPKEY_GET,
};
use crate::drivers::ble::mesh::model::foundation::configuration::beacon::{
    BeaconMessage, CONFIG_BEACON_GET, CONFIG_BEACON_SET,
};
use crate::drivers::ble::mesh::model::foundation::configuration::composition_data::{
    CompositionDataMessage, CONFIG_COMPOSITION_DATA_GET,
};
use crate::drivers::ble::mesh::model::foundation::configuration::default_ttl::{
    DefaultTTLMessage, CONFIG_DEFAULT_TTL_GET, CONFIG_DEFAULT_TTL_SET,
};
use crate::drivers::ble::mesh::model::foundation::configuration::model_app::{
    ModelAppMessage, CONFIG_MODEL_APP_BIND, CONFIG_MODEL_APP_UNBIND,
};
use crate::drivers::ble::mesh::model::foundation::configuration::model_publication::{
    ModelPublicationMessage, CONFIG_MODEL_PUBLICATION_SET,
    CONFIG_MODEL_PUBLICATION_VIRTUAL_ADDRESS_SET,
};

use crate::drivers::ble::mesh::model::foundation::configuration::model_subscription::{
    ModelSubscriptionMessage, CONFIG_MODEL_SUBSCRIPTION_ADD,
    CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_ADD,
};
use crate::drivers::ble::mesh::model::foundation::configuration::node_reset::{
    NodeResetMessage, CONFIG_NODE_RESET,
};
use crate::drivers::ble::mesh::model::{Message, Model, ModelIdentifier};
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use heapless::Vec;
use serde::{Deserialize, Serialize};

pub mod app_key;
pub mod beacon;
pub mod composition_data;
pub mod default_ttl;
pub mod model_app;
pub mod model_publication;
pub mod model_subscription;
pub mod node_reset;

pub const CONFIGURATION_SERVER: ModelIdentifier = ModelIdentifier::SIG(0x0000);
pub const CONFIGURATION_CLIENT: ModelIdentifier = ModelIdentifier::SIG(0x0001);

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ConfigurationMessage {
    Beacon(BeaconMessage),
    DefaultTTL(DefaultTTLMessage),
    NodeReset(NodeResetMessage),
    CompositionData(CompositionDataMessage),
    AppKey(AppKeyMessage),
    ModelApp(ModelAppMessage),
    ModelPublication(ModelPublicationMessage),
    ModelSubscription(ModelSubscriptionMessage),
}

impl Message for ConfigurationMessage {
    fn opcode(&self) -> Opcode {
        match self {
            ConfigurationMessage::Beacon(inner) => inner.opcode(),
            ConfigurationMessage::DefaultTTL(inner) => inner.opcode(),
            ConfigurationMessage::NodeReset(inner) => inner.opcode(),
            ConfigurationMessage::CompositionData(inner) => inner.opcode(),
            ConfigurationMessage::AppKey(inner) => inner.opcode(),
            ConfigurationMessage::ModelApp(inner) => inner.opcode(),
            ConfigurationMessage::ModelPublication(inner) => inner.opcode(),
            ConfigurationMessage::ModelSubscription(inner) => inner.opcode(),
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        match self {
            ConfigurationMessage::Beacon(inner) => inner.emit_parameters(xmit),
            ConfigurationMessage::DefaultTTL(inner) => inner.emit_parameters(xmit),
            ConfigurationMessage::NodeReset(inner) => inner.emit_parameters(xmit),
            ConfigurationMessage::CompositionData(inner) => inner.emit_parameters(xmit),
            ConfigurationMessage::AppKey(inner) => inner.emit_parameters(xmit),
            ConfigurationMessage::ModelApp(inner) => inner.emit_parameters(xmit),
            ConfigurationMessage::ModelPublication(inner) => inner.emit_parameters(xmit),
            ConfigurationMessage::ModelSubscription(inner) => inner.emit_parameters(xmit),
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
    const IDENTIFIER: ModelIdentifier = CONFIGURATION_SERVER;

    type Message<'m> = ConfigurationMessage;

    fn parse<'m>(
        &self,
        opcode: Opcode,
        parameters: &'m [u8],
    ) -> Result<Option<Self::Message<'m>>, ParseError> {
        match opcode {
            CONFIG_BEACON_GET => Ok(Some(ConfigurationMessage::Beacon(
                BeaconMessage::parse_get(parameters)?,
            ))),
            CONFIG_BEACON_SET => Ok(Some(ConfigurationMessage::Beacon(
                BeaconMessage::parse_set(parameters)?,
            ))),
            CONFIG_DEFAULT_TTL_GET => Ok(Some(ConfigurationMessage::DefaultTTL(
                DefaultTTLMessage::parse_get(parameters)?,
            ))),
            CONFIG_DEFAULT_TTL_SET => Ok(Some(ConfigurationMessage::DefaultTTL(
                DefaultTTLMessage::parse_set(parameters)?,
            ))),
            CONFIG_NODE_RESET => Ok(Some(ConfigurationMessage::NodeReset(
                NodeResetMessage::parse_reset(parameters)?,
            ))),
            CONFIG_COMPOSITION_DATA_GET => Ok(Some(ConfigurationMessage::CompositionData(
                CompositionDataMessage::parse_get(parameters)?,
            ))),
            // App Key
            CONFIG_APPKEY_ADD => Ok(Some(ConfigurationMessage::AppKey(
                AppKeyMessage::parse_add(parameters)?,
            ))),
            CONFIG_APPKEY_GET => Ok(Some(ConfigurationMessage::AppKey(
                AppKeyMessage::parse_get(parameters)?,
            ))),
            // Model App
            CONFIG_MODEL_APP_BIND => Ok(Some(ConfigurationMessage::ModelApp(
                ModelAppMessage::parse_bind(parameters)?,
            ))),
            CONFIG_MODEL_APP_UNBIND => Ok(Some(ConfigurationMessage::ModelApp(
                ModelAppMessage::parse_unbind(parameters)?,
            ))),
            // Model Publication
            CONFIG_MODEL_PUBLICATION_SET => Ok(Some(ConfigurationMessage::ModelPublication(
                ModelPublicationMessage::parse_set(parameters)?,
            ))),
            CONFIG_MODEL_PUBLICATION_VIRTUAL_ADDRESS_SET => {
                Ok(Some(ConfigurationMessage::ModelPublication(
                    ModelPublicationMessage::parse_virtual_address_set(parameters)?,
                )))
            }
            // Model Subscription
            CONFIG_MODEL_SUBSCRIPTION_ADD => Ok(Some(ConfigurationMessage::ModelSubscription(
                ModelSubscriptionMessage::parse_add(parameters)?,
            ))),
            CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_ADD => {
                Ok(Some(ConfigurationMessage::ModelSubscription(
                    ModelSubscriptionMessage::parse_virtual_address_add(parameters)?,
                )))
            }
            _ => Ok(None),
        }
    }
}

// ------------------------------------------------------------------------
// ------------------------------------------------------------------------

#[derive(Serialize, Deserialize, PartialEq, Copy, Clone)]
pub struct KeyIndex(u16);

#[cfg(feature = "defmt")]
impl defmt::Format for KeyIndex {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "{}", self.0);
    }
}

impl KeyIndex {
    fn parse_one(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() >= 2 {
            let byte1 = parameters[0];
            let byte2 = parameters[1] & 0b11110000 >> 4;
            let val = u16::from_be_bytes([byte2, byte1]);
            Ok(Self(val))
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    fn emit_one<const N: usize>(
        index: &KeyIndex,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        let bytes = index.0.to_be_bytes();
        let byte1 = bytes[1];
        let byte2 = bytes[0] << 4;
        xmit.push(byte1).map_err(|_| InsufficientBuffer)?;
        xmit.push(byte2).map_err(|_| InsufficientBuffer)?;
        Ok(())
    }

    fn parse_two(parameters: &[u8]) -> Result<(Self, Self), ParseError> {
        if parameters.len() >= 3 {
            let byte1 = parameters[0];
            let byte2 = (parameters[1] & 0b11110000) >> 4;

            let index1 = u16::from_be_bytes([byte1, byte2]);

            let byte1 = parameters[1] & 0b00001111;
            let byte2 = parameters[2];

            let index2 = u16::from_be_bytes([byte1, byte2]);
            Ok((Self(index2), Self(index1)))
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    fn emit_two<const N: usize>(
        indexes: (&KeyIndex, &KeyIndex),
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        let bytes = indexes.1 .0.to_be_bytes();
        let byte1 = bytes[0];
        xmit.push(byte1).map_err(|_| InsufficientBuffer)?;

        let byte2 = bytes[1] << 4;
        let bytes = indexes.0 .0.to_be_bytes();
        let byte1 = byte2 | bytes[0];

        xmit.push(byte1).map_err(|_| InsufficientBuffer)?;

        let byte2 = bytes[1];
        xmit.push(byte2).map_err(|_| InsufficientBuffer)?;

        Ok(())
    }
}

#[derive(Serialize, Deserialize, PartialEq, Copy, Clone)]
pub struct NetKeyIndex(KeyIndex);

impl NetKeyIndex {
    pub fn new(index: u16) -> Self {
        Self(KeyIndex(index))
    }

    fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        KeyIndex::emit_one(&self.0, xmit)
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for NetKeyIndex {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "{}", self.0)
    }
}

#[derive(Serialize, Deserialize, PartialEq, Copy, Clone)]
pub struct AppKeyIndex(KeyIndex);

impl AppKeyIndex {
    fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        KeyIndex::emit_one(&self.0, xmit)
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for AppKeyIndex {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "{}", self.0)
    }
}

#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct NetKeyAppKeyIndexesPair(NetKeyIndex, AppKeyIndex);

impl NetKeyAppKeyIndexesPair {
    fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        KeyIndex::emit_two((&self.0 .0, &self.1 .0), xmit).map_err(|_| InsufficientBuffer)?;
        Ok(())
    }

    pub fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() == 3 {
            let (net_key, app_key) = KeyIndex::parse_two(parameters)?;
            Ok(Self(NetKeyIndex(net_key), AppKeyIndex(app_key)))
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    pub fn net_key(&self) -> NetKeyIndex {
        self.0
    }

    pub fn app_key(&self) -> AppKeyIndex {
        self.1
    }
}

// ------------------------------------------------------------------------
// ------------------------------------------------------------------------
