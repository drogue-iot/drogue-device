mod segmentation;

use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::pdu::lower::{
    LowerAccess, LowerAccessMessage, LowerControl, LowerControlMessage, LowerPDU, Opcode, SzMic,
};
use crate::drivers::ble::mesh::pdu::network::CleartextNetworkPDU;
use ccm::aead::Buffer;

use self::segmentation::Segmentation;
use crate::drivers::ble::mesh::address::Address;
use crate::drivers::ble::mesh::app::ApplicationKeyIdentifier;
use crate::drivers::ble::mesh::crypto::nonce::{ApplicationNonce, DeviceNonce};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::authentication::AuthenticationContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::replay_cache::ReplayCache;
use crate::drivers::ble::mesh::pdu::upper::{UpperAccess, UpperPDU};
use core::future::Future;
use heapless::Vec;

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

    fn encrypt_application_key(
        &self,
        aid: ApplicationKeyIdentifier,
        nonce: ApplicationNonce,
        bytes: &mut [u8],
        mic: &mut [u8],
        additional_data: Option<&[u8]>,
    ) -> Result<(), DeviceError>;

    type NextSequenceFuture<'m>: Future<Output = Result<u32, DeviceError>> + 'm
    where
        Self: 'm;

    fn next_sequence<'m>(&'m self) -> Self::NextSequenceFuture<'m>;

    fn default_ttl(&self) -> u8;
}

pub struct Lower {
    replay_cache: ReplayCache,
    segmentation: Segmentation,
}

impl Default for Lower {
    fn default() -> Self {
        Self {
            replay_cache: Default::default(),
            segmentation: Default::default(),
        }
    }
}

const SEGMENTED_ACCESS_MTU: usize = 12;
const NONSEGMENTED_ACCESS_MUT: usize = 15;

impl Lower {
    pub async fn process_inbound<C: LowerContext>(
        &mut self,
        ctx: &C,
        pdu: CleartextNetworkPDU,
    ) -> Result<(Option<CleartextNetworkPDU>, Option<UpperPDU>), DeviceError> {
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
                            if self.replay_cache.has_seen(
                                ctx.iv_index().unwrap_or(0),
                                pdu.seq,
                                pdu.src,
                            ) {
                                return Ok((None, None));
                            }
                            // decrypt with device key
                            let nonce = DeviceNonce::new(
                                SzMic::Bit32,
                                pdu.seq,
                                pdu.src,
                                pdu.dst,
                                ctx.iv_index().ok_or(DeviceError::CryptoError)?,
                            );
                            ctx.decrypt_device_key(nonce, &mut payload, &trans_mic)?;
                        }
                        Ok((
                            None,
                            Some(UpperPDU::Access(UpperAccess {
                                ttl: Some(pdu.ttl),
                                network_key: pdu.network_key,
                                ivi: pdu.ivi,
                                nid: pdu.nid,
                                akf: access.akf,
                                aid: access.aid,
                                src: pdu.src,
                                dst: pdu.dst,
                                payload,
                            })),
                        ))
                    }
                    LowerAccessMessage::Segmented {
                        szmic,
                        seq_zero,
                        seg_o,
                        seg_n,
                        segment_m,
                    } => {
                        let (block_ack, payload) = self
                            .segmentation
                            .process_inbound(pdu.src, seq_zero, seg_o, seg_n, &segment_m)?;

                        let mut parameters = Vec::new();
                        //parameters
                        //.push(0)
                        //.map_err(|_| DeviceError::InsufficientBuffer)?;
                        let ack_seq_zero = (seq_zero << 2).to_be_bytes();
                        parameters
                            .push(ack_seq_zero[0])
                            .map_err(|_| DeviceError::InsufficientBuffer)?;
                        parameters
                            .push(ack_seq_zero[1])
                            .map_err(|_| DeviceError::InsufficientBuffer)?;
                        let block_ack = block_ack.to_be_bytes();
                        parameters
                            .push(block_ack[0])
                            .map_err(|_| DeviceError::InsufficientBuffer)?;
                        parameters
                            .push(block_ack[1])
                            .map_err(|_| DeviceError::InsufficientBuffer)?;
                        parameters
                            .push(block_ack[2])
                            .map_err(|_| DeviceError::InsufficientBuffer)?;
                        parameters
                            .push(block_ack[3])
                            .map_err(|_| DeviceError::InsufficientBuffer)?;

                        let ack = CleartextNetworkPDU {
                            network_key: pdu.network_key,
                            ivi: pdu.ivi,
                            nid: pdu.nid,
                            ttl: 1,
                            seq: ctx.next_sequence().await?,
                            src: ctx.primary_unicast_address()?,
                            dst: pdu.src.into(),
                            transport_pdu: LowerPDU::Control(LowerControl {
                                opcode: Opcode::SegmentedAcknowledgement,
                                message: LowerControlMessage::Unsegmented { parameters },
                            }),
                        };

                        if let Some(payload) = payload {
                            // todo: DRY this code
                            let (payload, trans_mic) = match szmic {
                                SzMic::Bit32 => payload.split_at(payload.len() - 4),
                                SzMic::Bit64 => payload.split_at(payload.len() - 8),
                            };

                            let mut payload = Vec::from_slice(payload)
                                .map_err(|_| DeviceError::InsufficientBuffer)?;

                            let seq_auth = Self::seq_auth(
                                ctx.iv_index().ok_or(DeviceError::CryptoError)?,
                                pdu.seq,
                                seq_zero,
                            );

                            if self.replay_cache.has_seen(
                                ctx.iv_index().unwrap_or(0),
                                pdu.seq,
                                pdu.src,
                            ) {
                                return Ok((None, None));
                            }

                            if access.akf {
                                // decrypt with aid key
                            } else {
                                // decrypt with device key
                                let nonce = DeviceNonce::new(
                                    szmic,
                                    seq_auth,
                                    pdu.src,
                                    pdu.dst,
                                    ctx.iv_index().ok_or(DeviceError::CryptoError)?,
                                );
                                ctx.decrypt_device_key(nonce, &mut payload, &trans_mic)?;
                            }
                            Ok((
                                Some(ack),
                                Some(UpperPDU::Access(UpperAccess {
                                    ttl: Some(pdu.ttl),
                                    network_key: pdu.network_key,
                                    ivi: pdu.ivi,
                                    nid: pdu.nid,
                                    akf: access.akf,
                                    aid: access.aid,
                                    src: pdu.src,
                                    dst: pdu.dst,
                                    payload,
                                })),
                            ))
                        } else {
                            Ok((Some(ack), None))
                        }
                    }
                }
            }
            LowerPDU::Control(control) => match control.message {
                LowerControlMessage::Unsegmented { .. } => {
                    //todo!("inbound unsegmented control")
                    Ok((None, None))
                }
                LowerControlMessage::Segmented { .. } => {
                    //todo!("inbound segmented control")
                    Ok((None, None))
                }
            },
        }
    }

    fn seq_auth(iv_index: u32, seq: u32, seq_zero: u16) -> u32 {
        (iv_index << 24) + Self::first_seq_number(seq, seq_zero)
    }

    fn first_seq_number(seq: u32, seq_zero: u16) -> u32 {
        if (seq & 8191u32) < seq_zero as u32 {
            seq - ((seq & 8191) - seq_zero as u32) - (8191 + 1)
        } else {
            seq - ((seq & 8191) - seq_zero as u32)
        }
    }

    pub async fn process_outbound<C: LowerContext>(
        &mut self,
        ctx: &C,
        pdu: UpperPDU,
    ) -> Result<Option<CleartextNetworkPDUSegments>, DeviceError> {
        match pdu {
            UpperPDU::Control(_control) => Ok(None),
            UpperPDU::Access(access) => {
                let mut payload: Vec<u8, 380> = Vec::from_slice(&access.payload)
                    .map_err(|_| DeviceError::InsufficientBuffer)?;

                let seq_zero = ctx.next_sequence().await?;

                let ttl = access.ttl.unwrap_or(ctx.default_ttl());

                let (akf, aid) = if access.akf {
                    let nonce = ApplicationNonce::new(
                        SzMic::Bit32,
                        seq_zero,
                        access.src,
                        access.dst,
                        ctx.iv_index().ok_or(DeviceError::CryptoError)?,
                    );
                    let mut trans_mic = [0; 4];

                    ctx.encrypt_application_key(
                        access.aid,
                        nonce,
                        &mut payload,
                        &mut trans_mic,
                        match &access.dst {
                            Address::LabelUuid(inner) => Some(inner.label_uuid()),
                            _ => None,
                        },
                    )?;
                    payload
                        .extend_from_slice(&trans_mic)
                        .map_err(|_| DeviceError::InsufficientBuffer)?;
                    (true, access.aid)
                } else {
                    // encrypt device key
                    let nonce = DeviceNonce::new(
                        SzMic::Bit32,
                        seq_zero,
                        access.src,
                        access.dst,
                        ctx.iv_index().ok_or(DeviceError::CryptoError)?,
                    );
                    let mut trans_mic = [0; 4];
                    ctx.encrypt_device_key(nonce, &mut payload, &mut trans_mic)?;
                    payload
                        .extend_from_slice(&trans_mic)
                        .map_err(|_| DeviceError::InsufficientBuffer)?;
                    (false, 0.into())
                };

                if payload.len() > NONSEGMENTED_ACCESS_MUT {
                    let payload = payload.chunks(SEGMENTED_ACCESS_MTU);

                    let mut segments = CleartextNetworkPDUSegments::new_empty();

                    let seg_n = payload.len() - 1;

                    for (seg_o, segment_m) in payload.enumerate() {
                        let seq = if seg_o == 0 {
                            seq_zero
                        } else {
                            ctx.next_sequence().await?
                        };
                        segments.add(CleartextNetworkPDU {
                            network_key: access.network_key,
                            ivi: access.ivi,
                            nid: access.nid,
                            ttl,
                            seq,
                            src: access.src,
                            dst: access.dst,
                            transport_pdu: LowerPDU::Access(LowerAccess {
                                akf,
                                aid,
                                message: LowerAccessMessage::Segmented {
                                    szmic: SzMic::Bit32,
                                    seq_zero: seq_zero as u16,
                                    seg_o: seg_o as u8,
                                    seg_n: seg_n as u8,
                                    segment_m: Vec::from_slice(segment_m).unwrap(),
                                },
                            }),
                        })?;
                    }
                    Ok(Some(segments))
                } else {
                    let payload =
                        Vec::from_slice(&payload).map_err(|_| DeviceError::InsufficientBuffer)?;
                    // can ship unsegmented
                    Ok(Some(CleartextNetworkPDUSegments::new(
                        CleartextNetworkPDU {
                            network_key: access.network_key,
                            ivi: access.ivi,
                            nid: access.nid,
                            ttl,
                            seq: seq_zero,
                            src: access.src,
                            dst: access.dst,
                            transport_pdu: LowerPDU::Access(LowerAccess {
                                akf,
                                aid,
                                message: LowerAccessMessage::Unsegmented(payload),
                            }),
                        },
                    )))
                }
            }
        }
    }
}

pub struct CleartextNetworkPDUSegments {
    segments: Vec<CleartextNetworkPDU, 10>,
}

impl CleartextNetworkPDUSegments {
    fn new(first: CleartextNetworkPDU) -> Self {
        let mut segments = Vec::new();
        segments.push(first).ok();
        Self { segments }
    }

    fn new_empty() -> Self {
        Self {
            segments: Default::default(),
        }
    }

    fn add(&mut self, pdu: CleartextNetworkPDU) -> Result<(), DeviceError> {
        self.segments
            .push(pdu)
            .map_err(|_| DeviceError::InsufficientBuffer)
    }

    pub fn iter(&self) -> CleartextNetworkPDUSegmentsIter {
        CleartextNetworkPDUSegmentsIter::new(self)
    }
}

pub struct CleartextNetworkPDUSegmentsIter<'a> {
    segments: &'a CleartextNetworkPDUSegments,
    cur: u8,
}

impl<'a> CleartextNetworkPDUSegmentsIter<'a> {
    fn new(segments: &'a CleartextNetworkPDUSegments) -> Self {
        Self { segments, cur: 0 }
    }
}

impl<'a> Iterator for CleartextNetworkPDUSegmentsIter<'a> {
    type Item = &'a CleartextNetworkPDU;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur >= self.segments.segments.len() as u8 {
            None
        } else {
            let cur = self.cur;
            self.cur = self.cur + 1;
            Some(&self.segments.segments[cur as usize])
        }
    }
}
