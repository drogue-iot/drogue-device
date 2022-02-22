use crate::drivers::ble::mesh::model::foundation::configuration::{
    AppKeyIndex, KeyIndex, NetKeyAppKeyIndexesPair, NetKeyIndex,
};
use crate::drivers::ble::mesh::model::{Message, Status};
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use crate::opcode;
use core::convert::TryInto;
use heapless::Vec;

opcode!( CONFIG_APPKEY_ADD 0x00 );
opcode!( CONFIG_APPKEY_DELETE 0x80, 0x00 );
opcode!( CONFIG_APPKEY_GET 0x80, 0x01 );
opcode!( CONFIG_APPKEY_LIST 0x80, 0x02 );
opcode!( CONFIG_APPKEY_STATUS 0x80, 0x03 );
opcode!( CONFIG_APPKEY_UPDATE 0x01 );

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
            let app_key = parameters[3..]
                .try_into()
                .map_err(|_| ParseError::InvalidLength)?;
            Ok(Self::Add(AppKeyAddMessage { indexes, app_key }))
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    pub fn parse_get(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() == 2 {
            let net_key_index = NetKeyIndex(KeyIndex::parse_one(parameters)?);
            Ok(Self::Get(AppKeyGetMessage { net_key_index }))
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

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
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

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AppKeyAddMessage {
    pub(crate) indexes: NetKeyAppKeyIndexesPair,
    pub(crate) app_key: [u8; 16],
}

impl AppKeyAddMessage {
    fn emit_parameters<const N: usize>(
        &self,
        _xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        todo!();
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AppKeyDeleteMessage {
    pub(crate) indexes: NetKeyAppKeyIndexesPair,
}

impl AppKeyDeleteMessage {
    fn emit_parameters<const N: usize>(
        &self,
        _xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        todo!();
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AppKeyGetMessage {
    pub(crate) net_key_index: NetKeyIndex,
}

impl AppKeyGetMessage {
    fn emit_parameters<const N: usize>(
        &self,
        _xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AppKeyListMessage {
    pub(crate) status: Status,
    pub(crate) net_key_index: NetKeyIndex,
    pub(crate) app_key_indexes: Vec<AppKeyIndex, 10>,
}

impl AppKeyListMessage {
    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.push(self.status as u8)
            .map_err(|_| InsufficientBuffer)?;
        self.net_key_index.emit(xmit)?;

        /*
        for (i, app_key_index) in self.app_key_indexes.iter().enumerate() {
            if (i + 1) % 2 == 0 {
                app_key_index.0.emit_second(xmit)?;
            } else {
                app_key_index.0.emit_first(xmit)?;
            }
        }
         */
        for chunk in self.app_key_indexes.chunks(2) {
            if chunk.len() == 2 {
                KeyIndex::emit_two((&chunk[0].0, &chunk[1].0), xmit)?;
            } else {
                KeyIndex::emit_one(&chunk[0].0, xmit)?;
            }
        }

        Ok(())
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AppKeyStatusMessage {
    pub(crate) status: Status,
    pub(crate) indexes: NetKeyAppKeyIndexesPair,
}

impl AppKeyStatusMessage {
    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.push(self.status as u8)
            .map_err(|_| InsufficientBuffer)?;
        self.indexes.emit(xmit)?;
        Ok(())
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AppKeyUpdateMessage {
    pub(crate) net_key_index: NetKeyIndex,
    pub(crate) app_key_index: AppKeyIndex,
    pub(crate) app_key: [u8; 16],
}

impl AppKeyUpdateMessage {
    fn emit_parameters<const N: usize>(
        &self,
        _xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}
