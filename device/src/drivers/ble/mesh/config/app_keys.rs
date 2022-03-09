use crate::drivers::ble::mesh::app::ApplicationKeyIdentifier;
use crate::drivers::ble::mesh::crypto;
use crate::drivers::ble::mesh::model::foundation::configuration::AppKeyIndex;
use crate::drivers::ble::mesh::model::Status;
use heapless::Vec;
use serde::{Deserialize, Serialize};

#[derive(Default)]
pub struct AppKeys {
    keys: Vec<AppKeyDetails, 10>,
}

impl AppKeys {
    fn find_by_aid(&self, aid: ApplicationKeyIdentifier) -> Option<&AppKeyDetails> {
        self.keys.iter().find(|e| e.aid == aid)
    }

    fn find_by_index(&self, index: AppKeyIndex) -> Option<&AppKeyDetails> {
        self.keys.iter().find(|e| e.index == index)
    }

    fn add(&mut self, index: AppKeyIndex, key: AppKey) -> Result<(), Status> {
        if let Some(_) = self.find_by_index(index) {
            Err(Status::KeyIndexAlreadyStored)
        } else {
            let aid = crypto::k4(key.as_ref())
                .map_err(|_| Status::UnspecifiedError)?
                .into();
            self.keys
                .push(AppKeyDetails { aid, key, index })
                .map_err(|_| Status::InsufficientResources)?;
            Ok(())
        }
    }

    fn remove(&mut self, index: AppKeyIndex) -> Result<(), Status> {
        let mut removed = false;
        let mut keys = Vec::new();

        for key in self.keys.iter() {
            if key.index != index {
                keys.push(*key).ok();
                removed = true;
            }
        }

        if removed {
            self.keys = keys;
            Ok(())
        } else {
            Err(Status::InvalidAppKeyIndex)
        }
    }
}

#[derive(Serialize, Deserialize, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AppKeyDetails {
    pub(crate) aid: ApplicationKeyIdentifier,
    pub(crate) key: AppKey,
    pub(crate) index: AppKeyIndex,
}

impl AppKeyDetails {
    #[cfg(feature = "defmt")]
    pub(crate) fn display_configuration(&self) {
        info!("  {}: {} [aid={}]", self.index, self.key, self.aid)
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Default)]
pub struct AppKey([u8; 16]);

impl AsRef<[u8; 16]> for AppKey {
    fn as_ref(&self) -> &[u8; 16] {
        &self.0
    }
}

impl From<[u8; 16]> for AppKey {
    fn from(val: [u8; 16]) -> Self {
        Self(val)
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for AppKey {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7], self.0[8], self.0[9], self.0[10], self.0[11], self.0[12], self.0[13], self.0[14], self.0[15],
        )
    }
}
