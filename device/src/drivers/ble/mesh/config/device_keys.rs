use crate::drivers::ble::mesh::driver::DeviceError;
use core::convert::TryInto;
use p256::ecdh::SharedSecret;
use p256::elliptic_curve::generic_array::{typenum::consts::U32, GenericArray};
use p256::{PublicKey, SecretKey};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DeviceKeys {
    random: Option<[u8; 16]>,
    private_key: Option<[u8; 32]>,
    shared_secret: Option<[u8; 32]>,
    device_key: Option<DeviceKey>,
}

impl DeviceKeys {
    #[cfg(feature = "defmt")]
    pub(crate) fn display_configuration(&self) {
        if let Some(key) = self.device_key {
            info!("DeviceKey: {}", key);
        } else {
            info!("DeviceKey: None");
        }
    }

    pub(crate) fn private_key(&self) -> Result<Option<SecretKey>, DeviceError> {
        match self.private_key {
            None => Ok(None),
            Some(private_key) => Ok(Some(
                SecretKey::from_be_bytes(&private_key).map_err(|_| DeviceError::Serialization)?,
            )),
        }
    }

    pub(crate) fn set_private_key(
        &mut self,
        private_key: &Option<SecretKey>,
    ) -> Result<(), DeviceError> {
        match private_key {
            None => {
                self.private_key.take();
            }
            Some(private_key) => {
                self.private_key.replace(
                    private_key
                        .to_nonzero_scalar()
                        .to_bytes()
                        .try_into()
                        .map_err(|_| DeviceError::Serialization)?,
                );
            }
        }
        Ok(())
    }

    pub(crate) fn public_key(&self) -> Result<PublicKey, DeviceError> {
        Ok(self
            .private_key()?
            .ok_or(DeviceError::KeyInitialization)?
            .public_key())
    }

    pub(crate) fn shared_secret(&self) -> Result<Option<SharedSecret>, DeviceError> {
        match self.shared_secret {
            None => Ok(None),
            Some(shared_secret) => {
                let arr: GenericArray<u8, U32> = shared_secret.into();
                Ok(Some(SharedSecret::from(arr)))
            }
        }
    }

    pub(crate) fn set_shared_secret(
        &mut self,
        shared_secret: Option<SharedSecret>,
    ) -> Result<(), DeviceError> {
        match shared_secret {
            None => {
                self.shared_secret.take();
            }
            Some(shared_secret) => {
                let bytes = &shared_secret.as_bytes()[0..];
                self.shared_secret.replace(
                    bytes
                        .try_into()
                        .map_err(|_| DeviceError::InvalidKeyLength)?,
                );
            }
        }
        Ok(())
    }

    /*
    pub(crate) fn set_provisioning_salt(
        &mut self,
        provisioning_salt: [u8; 16],
    ) -> Result<(), DeviceError> {
        self.provisioning_salt.replace(provisioning_salt);
        Ok(())
    }

    pub(crate) fn provisioning_salt(&self) -> Result<Option<[u8; 16]>, DeviceError> {
        Ok(self.provisioning_salt)
    }
     */

    pub(crate) fn set_device_key(&mut self, key: [u8; 16]) {
        self.device_key.replace(DeviceKey::new(key));
    }

    pub(crate) fn device_key(&self) -> Option<DeviceKey> {
        self.device_key
    }
}

#[derive(Serialize, Deserialize, Copy, Clone, Default)]
pub struct DeviceKey([u8; 16]);

impl DeviceKey {
    pub fn new(material: [u8; 16]) -> Self {
        Self(material)
    }
}

impl AsRef<[u8; 16]> for DeviceKey {
    fn as_ref(&self) -> &[u8; 16] {
        &self.0
    }
}

#[cfg(feature = "defmt")]
impl defmt::Format for DeviceKey {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7], self.0[8], self.0[9], self.0[10], self.0[11], self.0[12], self.0[13], self.0[14], self.0[15],
        )
    }
}
