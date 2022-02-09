use core::convert::TryInto;
use crate::drivers::ble::mesh::composition::Composition;
use crate::drivers::ble::mesh::model::{FoundationIdentifier, Message, Model, ModelIdentifier, Status};
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use crate::opcode;
use defmt::Format;
use heapless::Vec;

#[derive(Format)]
pub enum ConfigurationMessage {
    Beacon(BeaconMessage),
    DefaultTTL(DefaultTTLMessage),
    NodeReset(NodeResetMessage),
    CompositionData(CompositionDataMessage),
    AppKey(AppKeyMessage),
}

impl Message for ConfigurationMessage {
    fn opcode(&self) -> Opcode {
        match self {
            ConfigurationMessage::Beacon(inner) => inner.opcode(),
            ConfigurationMessage::DefaultTTL(inner) => inner.opcode(),
            ConfigurationMessage::NodeReset(inner) => inner.opcode(),
            ConfigurationMessage::CompositionData(inner) => inner.opcode(),
            ConfigurationMessage::AppKey(inner) => inner.opcode(),
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

    fn parse(
        &self,
        opcode: Opcode,
        parameters: &[u8],
    ) -> Result<Option<Self::MESSAGE>, ParseError> {
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

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        match self {
            Self::Get => {}
            Self::Set(val) => xmit
                .push(if *val { 1 } else { 0 })
                .map_err(|_| InsufficientBuffer)?,
            Self::Status(val) => xmit
                .push(if *val { 1 } else { 0 })
                .map_err(|_| InsufficientBuffer)?,
        }
        Ok(())
    }
}

#[allow(unused)]
impl BeaconMessage {
    pub fn parse_get(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.is_empty() {
            Ok(Self::Get)
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    pub fn parse_set(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() == 1 {
            if parameters[0] == 0x00 {
                Ok(Self::Set(false))
            } else if parameters[0] == 0x01 {
                Ok(Self::Set(true))
            } else {
                Err(ParseError::InvalidValue)
            }
        } else {
            Err(ParseError::InvalidLength)
        }
    }
}

opcode!( CONFIG_DEFAULT_TTL_GET 0x80, 0x0C );
opcode!( CONFIG_DEFAULT_TTL_SET 0x80, 0x0D );
opcode!( CONFIG_DEFAULT_TTL_STATUS 0x80, 0x0E );

#[derive(Format)]
pub enum DefaultTTLMessage {
    Get,
    Set(u8),
    Status(u8),
}

#[allow(unused)]
impl Message for DefaultTTLMessage {
    fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_DEFAULT_TTL_GET,
            Self::Set(_) => CONFIG_DEFAULT_TTL_SET,
            Self::Status(_) => CONFIG_DEFAULT_TTL_STATUS,
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        match self {
            Self::Get => {}
            Self::Set(val) => xmit.push(*val).map_err(|_| InsufficientBuffer)?,
            Self::Status(val) => xmit.push(*val).map_err(|_| InsufficientBuffer)?,
        }
        Ok(())
    }
}

impl DefaultTTLMessage {
    pub fn parse_get(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.is_empty() {
            Ok(Self::Get)
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    pub fn parse_set(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() == 1 {
            Ok(Self::Set(parameters[0]))
        } else {
            Err(ParseError::InvalidLength)
        }
    }
}

opcode!( CONFIG_NODE_RESET 0x80, 0x49 );
opcode!( CONFIG_NODE_RESET_STATUS 0x80, 0x4A );

#[derive(Format)]
pub enum NodeResetMessage {
    Reset,
    Status,
}

#[allow(unused)]
impl NodeResetMessage {
    pub fn parse_reset(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.is_empty() {
            Ok(Self::Reset)
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    pub fn parse_status(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.is_empty() {
            Ok(Self::Status)
        } else {
            Err(ParseError::InvalidLength)
        }
    }
}

impl Message for NodeResetMessage {
    fn opcode(&self) -> Opcode {
        match self {
            Self::Reset => CONFIG_NODE_RESET,
            Self::Status => CONFIG_NODE_RESET_STATUS,
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        _xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        Ok(())
    }
}

opcode!( CONFIG_COMPOSITION_DATA_GET 0x80, 0x08 );
opcode!( CONFIG_COMPOSITION_DATA_STATUS 0x02 );

#[derive(Format)]
pub enum CompositionDataMessage {
    Get(u8),
    Status(CompositionStatus),
}

#[allow(unused)]
impl CompositionDataMessage {
    fn parse_get(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() == 1 {
            Ok(Self::Get(parameters[0]))
        } else {
            Err(ParseError::InvalidLength)
        }
    }
}

impl Message for CompositionDataMessage {
    fn opcode(&self) -> Opcode {
        match self {
            Self::Get(_) => CONFIG_COMPOSITION_DATA_GET,
            Self::Status(_) => CONFIG_COMPOSITION_DATA_STATUS,
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        match self {
            CompositionDataMessage::Get(page) => {
                xmit.push(*page).map_err(|_| InsufficientBuffer)?
            }
            CompositionDataMessage::Status(inner) => inner.emit_parameters(xmit)?,
        }
        Ok(())
    }
}

#[derive(Format)]
pub struct CompositionStatus {
    pub(crate) page: u8,
    pub(crate) data: Composition,
}

impl CompositionStatus {
    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.push(self.page).map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.data.cid.0.to_be_bytes())
            .map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.data.pid.0.to_be_bytes())
            .map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.data.vid.0.to_be_bytes())
            .map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.data.crpl.to_be_bytes())
            .map_err(|_| InsufficientBuffer)?;
        self.data.features.emit(xmit)?;
        for element in self.data.elements.iter() {
            xmit.extend_from_slice(&element.loc.0.to_be_bytes())
                .map_err(|_| InsufficientBuffer)?;
            let sig_models: Vec<_, 10> = element
                .models
                .iter()
                .filter(|e| matches!(e, ModelIdentifier::SIG(_)))
                .collect();
            let vendor_models: Vec<_, 10> = element
                .models
                .iter()
                .filter(|e| matches!(e, ModelIdentifier::Vendor(..)))
                .collect();

            xmit.push(sig_models.len() as u8)
                .map_err(|_| InsufficientBuffer)?;
            xmit.push(vendor_models.len() as u8)
                .map_err(|_| InsufficientBuffer)?;

            for model in sig_models.iter() {
                model.emit(xmit)?
            }

            for model in vendor_models.iter() {
                model.emit(xmit)?
            }
        }
        Ok(())
    }
}

opcode!( CONFIG_APPKEY_ADD 0x00 );
opcode!( CONFIG_APPKEY_DELETE 0x80, 0x00 );
opcode!( CONFIG_APPKEY_GET 0x80, 0x01 );
opcode!( CONFIG_APPKEY_LIST 0x80, 0x02 );
opcode!( CONFIG_APPKEY_STATUS 0x80, 0x03 );
opcode!( CONFIG_APPKEY_UPDATE 0x01 );

#[derive(Format)]
pub enum AppKeyMessage {
    Add(AppKeyAddMessage),
    Delete(AppKeyDeleteMessage),
    Get(AppKeyGetMessage),
    List(AppKeyListMessage),
    Status(AppKeyStatusMessage),
    Update(AppKeyUpdateMessage),
}

impl AppKeyMessage {
    pub fn parse_add(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() == 19 {
            let indexes = NetKeyAppKeyIndexesPair::parse(&parameters[0..=2])?;
            let app_key = parameters[3..].try_into().map_err(|_| ParseError::InvalidLength)?;
            Ok(
                Self::Add(
                    AppKeyAddMessage {
                        indexes,
                        app_key,
                    }
                )
            )
        } else {
            Err(ParseError::InvalidLength)
        }

    }
}

impl Message for AppKeyMessage {
    fn opcode(&self) -> Opcode {
        match self {
            Self::Add(_) => CONFIG_APPKEY_ADD,
            Self::Delete(_) => CONFIG_APPKEY_DELETE,
            Self::Get(_) => CONFIG_APPKEY_GET,
            Self::List(_) => CONFIG_APPKEY_LIST,
            Self::Status(_) => CONFIG_APPKEY_STATUS,
            Self::Update(_) => CONFIG_APPKEY_STATUS,
        }
    }

    fn emit_parameters<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        match self {
            AppKeyMessage::Add(inner) => inner.emit_parameters(xmit),
            AppKeyMessage::Delete(inner) => inner.emit_parameters(xmit),
            AppKeyMessage::Get(inner) => inner.emit_parameters(xmit),
            AppKeyMessage::List(inner) => inner.emit_parameters(xmit),
            AppKeyMessage::Status(inner) => inner.emit_parameters(xmit),
            AppKeyMessage::Update(inner) => inner.emit_parameters(xmit),
        }
    }
}

#[derive(Format, Copy, Clone)]
pub struct NetKeyAppKeyIndexesPair(u16, u16);

impl NetKeyAppKeyIndexesPair {
    fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        let net_key = self.0.to_be_bytes();
        let app_key = self.1.to_be_bytes();
        xmit.push(((net_key[0] & 0b00001111)  <<  4) | net_key[1] & 0b11110000).map_err(|_|InsufficientBuffer)?;
        xmit.push( ((net_key[1] & 0b00001111) << 4) | app_key[0] & 0b00001111).map_err(|_|InsufficientBuffer)?;
        xmit.push( app_key[1]).map_err(|_|InsufficientBuffer)?;
        Ok(())
    }

    pub fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() == 3 {
            let net_key = ((parameters[0] as u16) << 4) | ((parameters[1] as u16) & 0b11110000 >> 4);
            let app_key = (((parameters[1] as u16) & 0b00001111) << 8) | (parameters[2] as u16);
            Ok(
                Self(net_key, app_key)
            )
        } else {
            Err(ParseError::InvalidLength)
        }

    }

    pub fn net_key(&self) -> u16 {
        self.0
    }

    pub fn app_key(&self) -> u16 {
        self.1
    }
}


#[derive(Format)]
pub struct AppKeyAddMessage {
    pub(crate) indexes: NetKeyAppKeyIndexesPair,
    pub(crate) app_key: [u8; 16],
}

impl AppKeyAddMessage {
    fn emit_parameters<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!();
    }
}

#[derive(Format)]
pub struct AppKeyDeleteMessage {
    pub(crate) indexes: NetKeyAppKeyIndexesPair,
}

impl AppKeyDeleteMessage {
    fn emit_parameters<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!();
    }

}

#[derive(Format)]
pub struct AppKeyGetMessage {
    pub(crate) net_key_index: u16,
}

impl AppKeyGetMessage {
    fn emit_parameters<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }

}

#[derive(Format)]
pub struct AppKeyListMessage {
    pub(crate) status: Status,
    pub(crate) net_key_index: u16,
    pub(crate) app_key_indexes: Vec<u16, 10>,
}

impl AppKeyListMessage {
    fn emit_parameters<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub struct AppKeyStatusMessage {
    pub(crate) status: Status,
    pub(crate) indexes: NetKeyAppKeyIndexesPair,
}

impl AppKeyStatusMessage {
    fn emit_parameters<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        xmit.push( self.status as u8 ).map_err(|_|InsufficientBuffer)?;
        self.indexes.emit(xmit)?;
        Ok(())
    }
}

#[derive(Format)]
pub struct AppKeyUpdateMessage {
    pub(crate) net_key_index: u16,
    pub(crate) app_key_index: u16,
    pub(crate) app_key: [u8; 16],
}

impl AppKeyUpdateMessage {
    fn emit_parameters<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}
