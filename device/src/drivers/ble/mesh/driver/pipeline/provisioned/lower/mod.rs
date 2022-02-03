use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::pdu::lower::{
    LowerAccess, LowerAccessMessage, LowerControlMessage, LowerPDU,
};
use crate::drivers::ble::mesh::pdu::network::CleartextNetworkPDU;
use ccm::aead::Buffer;

use crate::drivers::ble::mesh::crypto::nonce::DeviceNonce;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::authentication::AuthenticationContext;
use crate::drivers::ble::mesh::pdu::upper::{UpperAccess, UpperPDU};
use heapless::Vec;
use core::future::Future;

pub trait LowerContext: AuthenticationContext {
    fn decrypt_device_key(
        &self,
        nonce: DeviceNonce,
        bytes: &mut [u8],
        mic: &[u8],
    ) -> Result<(), DeviceError>;
    fn encrypt_device_key(
        &self,
        nonce: DeviceNonce,
        bytes: &mut [u8],
        mic: &mut [u8],
    ) -> Result<(), DeviceError>;

    type NextSequenceFuture<'m>: Future<Output = Result<u32, DeviceError>> + 'm
    where
        Self: 'm;

    fn next_sequence<'m>(&'m self) -> Self::NextSequenceFuture<'m>;
}

pub struct Lower {}

impl Default for Lower {
    fn default() -> Self {
        Self {}
    }
}

impl Lower {
    pub async fn process_inbound<C: LowerContext>(
        &mut self,
        ctx: &C,
        pdu: CleartextNetworkPDU,
    ) -> Result<Option<UpperPDU>, DeviceError> {
        match pdu.transport_pdu {
            LowerPDU::Access(access) => {
                match access.message {
                    LowerAccessMessage::Unsegmented(payload) => {
                        // TransMIC is 32 bits for unsegmented access messages.
                        let (payload, trans_mic) = payload.split_at(payload.len() - 4);
                        let mut payload = Vec::from_slice(payload)
                            .map_err(|_| DeviceError::InsufficientBuffer)?;

                        if access.akf {
                            // decrypt with aid key
                        } else {
                            // decrypt with device key
                            let nonce = DeviceNonce::new(
                                false,
                                pdu.seq,
                                pdu.src,
                                pdu.dst,
                                ctx.iv_index().ok_or(DeviceError::CryptoError)?,
                            );
                            ctx.decrypt_device_key(nonce, &mut payload, &trans_mic)?;
                        }
                        Ok(Some(UpperPDU::Access(UpperAccess {
                            network_key: pdu.network_key,
                            ivi: pdu.ivi,
                            nid: pdu.nid,
                            akf: access.akf,
                            aid: access.aid,
                            src: pdu.src,
                            dst: pdu.dst,
                            payload,
                        })))
                    }
                    LowerAccessMessage::Segmented { .. } => {
                        todo!()
                    }
                }
            }
            LowerPDU::Control(control) => match control.message {
                LowerControlMessage::Unsegmented { .. } => {
                    todo!()
                }
                LowerControlMessage::Segmented { .. } => {
                    todo!()
                }
            },
        }
    }

    pub async fn process_outbound<C: LowerContext>(
        &mut self,
        ctx: &C,
        pdu: UpperPDU,
    ) -> Result<Option<CleartextNetworkPDU>, DeviceError> {
        // todo: work with segmented
        match pdu {
            UpperPDU::Control(_control) => Ok(None),
            UpperPDU::Access(access) => {
                let mut payload = Vec::from_slice(&access.payload)
                    .map_err(|_| DeviceError::InsufficientBuffer)?;

                let seq = ctx.next_sequence().await?;

                if access.akf {
                    // encrypt with application key
                } else {
                    // encrypt device key
                    let nonce = DeviceNonce::new(
                        false,
                        seq,
                        access.src,
                        access.dst,
                        ctx.iv_index().ok_or(DeviceError::CryptoError)?,
                    );
                    let mut trans_mic = [0; 4];
                    ctx.encrypt_device_key(nonce, &mut payload, &mut trans_mic)?;

                    let mut check: Vec<u8, 15> = Vec::new();
                    check.extend_from_slice(&payload).ok();
                    ctx.decrypt_device_key(nonce, &mut check, &trans_mic)?;

                    payload
                        .extend_from_slice(&trans_mic)
                        .map_err(|_| DeviceError::InsufficientBuffer)?;
                }
                Ok(Some(CleartextNetworkPDU {
                    network_key: access.network_key,
                    ivi: access.ivi,
                    nid: access.nid,
                    ttl: 127,
                    seq,
                    src: access.src,
                    dst: access.dst,
                    transport_pdu: LowerPDU::Access(LowerAccess {
                        akf: false,
                        aid: 0,
                        message: LowerAccessMessage::Unsegmented(payload),
                    }),
                }))
            }
        }
    }
}
