mod auth_value;
mod transcript;

use crate::drivers::ble::mesh::driver::pipeline::mesh::MeshContext;
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::provisionable::auth_value::{
    determine_auth_value, AuthValue,
};
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::provisionable::transcript::Transcript;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::{
    Capabilities, Confirmation, ProvisioningData, ProvisioningPDU, PublicKey, Random,
};
use aes::Aes128;
use cmac::crypto_mac::Output;
use cmac::Cmac;
use core::convert::TryFrom;
use core::future::Future;
use heapless::Vec;
use p256::elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint};
use p256::EncodedPoint;

pub trait UnprovisionedContext: MeshContext {
    fn rng_fill(&self, dest: &mut [u8]);

    type SetPeerPublicKeyFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn set_peer_public_key<'m>(&'m self, pk: p256::PublicKey) -> Self::SetPeerPublicKeyFuture<'m>;

    fn public_key(&self) -> Result<p256::PublicKey, DeviceError>;

    type SetProvisioningDataFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn set_provisioning_data<'m>(
        &'m self,
        provisioning_salt: &'m [u8],
        data: &'m ProvisioningData,
    ) -> Self::SetProvisioningDataFuture<'m>;

    fn aes_cmac(&self, key: &[u8], input: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError>;

    fn s1(&self, input: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError>;

    fn prsk(&self, salt: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError>;
    fn prsn(&self, salt: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError>;
    fn prck(&self, salt: &[u8]) -> Result<Output<Cmac<Aes128>>, DeviceError>;

    fn aes_ccm_decrypt(
        &self,
        key: &[u8],
        nonce: &[u8],
        data: &mut [u8],
        mic: &[u8],
        additional_data: Option<&[u8]>,
    ) -> Result<(), DeviceError>;

    fn rng_u8(&self) -> u8;
    fn rng_u32(&self) -> u32;
}

pub struct Provisionable {
    capabilities: Capabilities,
    transcript: Transcript,
    auth_value: Option<AuthValue>,
    random_device: Option<[u8; 16]>,
    random_provisioner: Option<[u8; 16]>,
}

impl Provisionable {
    pub fn new(capabilities: Capabilities) -> Self {
        Self {
            capabilities,
            transcript: Transcript::default(),
            auth_value: None,
            random_device: None,
            random_provisioner: None,
        }
    }

    pub fn reset(&mut self) {
        self.transcript.reset();
        self.auth_value.take();
        self.random_device.take();
        self.random_provisioner.take();
    }

    pub async fn process_inbound<C: UnprovisionedContext>(
        &mut self,
        ctx: &C,
        pdu: ProvisioningPDU,
    ) -> Result<Option<ProvisioningPDU>, DeviceError> {
        match pdu {
            ProvisioningPDU::Invite(invite) => {
                self.transcript.add_invite(&invite)?;
                self.transcript.add_capabilities(&self.capabilities)?;
                Ok(Some(ProvisioningPDU::Capabilities(
                    self.capabilities.clone(),
                )))
            }
            ProvisioningPDU::Capabilities(_) => Ok(None),
            ProvisioningPDU::Start(start) => {
                self.transcript.add_start(&start)?;
                let auth_value = determine_auth_value(ctx, &start)?;
                // TODO actually let the device/app/thingy know what it is so that it can blink/flash/accept input
                self.auth_value.replace(auth_value);
                Ok(None)
            }
            ProvisioningPDU::PublicKey(public_key) => {
                self.transcript.add_pubkey_provisioner(&public_key)?;
                let peer_pk_x = public_key.x;
                let peer_pk_y = public_key.y;

                // TODO remove unwrap
                let peer_pk =
                    p256::PublicKey::from_encoded_point(&EncodedPoint::from_affine_coordinates(
                        &peer_pk_x.into(),
                        &peer_pk_y.into(),
                        false,
                    ))
                    .unwrap();

                ctx.set_peer_public_key(peer_pk).await?;
                let pk = ctx.public_key()?;
                let xy = pk.to_encoded_point(false);
                let x = xy.x().unwrap();
                let y = xy.y().unwrap();
                let pk = PublicKey {
                    x: <[u8; 32]>::try_from(x.as_slice())
                        .map_err(|_| DeviceError::InsufficientBuffer)?,
                    y: <[u8; 32]>::try_from(y.as_slice())
                        .map_err(|_| DeviceError::InsufficientBuffer)?,
                };
                self.transcript.add_pubkey_device(&pk)?;
                Ok(Some(ProvisioningPDU::PublicKey(pk)))
            }
            ProvisioningPDU::InputComplete => Ok(None),
            ProvisioningPDU::Confirmation(_confirmation) => {
                // todo verify the confirmation from the provisioner.
                let mut random_device = [0; 16];
                ctx.rng_fill(&mut random_device);
                self.random_device.replace(random_device);
                let confirmation_device = self.confirmation_device(ctx)?;
                Ok(Some(ProvisioningPDU::Confirmation(confirmation_device)))
            }
            ProvisioningPDU::Random(random) => {
                self.random_provisioner.replace(random.random);
                Ok(Some(ProvisioningPDU::Random(Random {
                    random: self
                        .random_device
                        .ok_or(DeviceError::CryptoError("provisioning random"))?,
                })))
            }
            ProvisioningPDU::Data(mut data) => {
                let mut provisioning_salt = [0; 48];
                provisioning_salt[0..16]
                    .copy_from_slice(&self.transcript.confirmation_salt()?.into_bytes());
                provisioning_salt[16..32]
                    .copy_from_slice(self.random_provisioner.as_ref().unwrap());
                provisioning_salt[32..48].copy_from_slice(self.random_device.as_ref().unwrap());
                let provisioning_salt = &ctx.s1(&provisioning_salt)?.into_bytes()[0..];

                let session_key = &ctx.prsk(&provisioning_salt)?.into_bytes()[0..];
                let session_nonce = &ctx.prsn(&provisioning_salt)?.into_bytes()[3..];

                let result = ctx.aes_ccm_decrypt(
                    &session_key,
                    &session_nonce,
                    &mut data.encrypted,
                    &data.mic,
                    None,
                );
                match result {
                    Ok(_) => {
                        let provisioning_data = ProvisioningData::parse(&data.encrypted)?;
                        ctx.set_provisioning_data(&provisioning_salt, &provisioning_data)
                            .await?;
                    }
                    Err(_) => {
                        return Err(DeviceError::CryptoError("provisioning data"));
                    }
                }
                Ok(Some(ProvisioningPDU::Complete))
            }
            ProvisioningPDU::Complete => Ok(None),
            ProvisioningPDU::Failed(_) => Ok(None),
        }
    }

    fn confirmation_device<C: UnprovisionedContext>(
        &self,
        ctx: &C,
    ) -> Result<Confirmation, DeviceError> {
        let salt = self.transcript.confirmation_salt()?;
        //let confirmation_key = device.key_manager.borrow().k1(&*salt.into_bytes(), b"prck")?;
        let confirmation_key = ctx.prck(&*salt.into_bytes())?;
        let mut bytes: Vec<u8, 32> = Vec::new();
        bytes
            .extend_from_slice(&self.random_device.unwrap())
            .map_err(|_| DeviceError::InsufficientBuffer)?;
        bytes
            .extend_from_slice(
                &self
                    .auth_value
                    .as_ref()
                    .ok_or(DeviceError::InsufficientBuffer)?
                    .get_bytes(),
            )
            .map_err(|_| DeviceError::InsufficientBuffer)?;
        let confirmation_device = ctx.aes_cmac(&confirmation_key.into_bytes(), &bytes)?;

        let mut confirmation = [0; 16];
        for (i, byte) in confirmation_device.into_bytes().iter().enumerate() {
            confirmation[i] = *byte;
        }

        Ok(Confirmation { confirmation })
    }
}
