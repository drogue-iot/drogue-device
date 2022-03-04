use crate::drivers::ble::mesh::app::ApplicationKeyIdentifier;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use heapless::Vec;

#[derive(Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LowerPDU {
    Access(LowerAccess),
    Control(LowerControl),
}

impl LowerPDU {
    pub fn parse(ctl: bool, data: &[u8]) -> Result<Self, ParseError> {
        if data.len() >= 2 {
            let seg = data[0] & 0b10000000 != 0;

            match (ctl, seg) {
                (true, false) => Ok(LowerPDU::Control(Self::parse_unsegmented_control(data)?)),
                (true, true) => Ok(LowerPDU::Control(Self::parse_segmented_control(data)?)),
                (false, false) => Ok(LowerPDU::Access(Self::parse_unsegmented_access(data)?)),
                (false, true) => Ok(LowerPDU::Access(Self::parse_segmented_access(data)?)),
            }
        } else {
            Err(ParseError::InvalidLength)
        }
    }

    fn parse_unsegmented_control(data: &[u8]) -> Result<LowerControl, ParseError> {
        let opcode = Opcode::parse(data[0] & 0b01111111).ok_or(ParseError::InvalidValue)?;
        let parameters = &data[1..];
        Ok(LowerControl {
            opcode,
            message: LowerControlMessage::Unsegmented {
                parameters: Vec::from_slice(parameters)
                    .map_err(|_| ParseError::InsufficientBuffer)?,
            },
        })
    }

    fn parse_segmented_control(data: &[u8]) -> Result<LowerControl, ParseError> {
        let opcode = Opcode::parse(data[0] & 0b01111111).ok_or(ParseError::InvalidValue)?;
        let seq_zero = u16::from_be_bytes([data[1] & 0b01111111, data[2] & 0b11111100]) >> 2;
        let seg_o = (u16::from_be_bytes([data[2] & 0b00000011, data[3] & 0b11100000]) >> 5) as u8;
        let seg_n = data[3] & 0b00011111;
        let segment_m = &data[4..];
        Ok(LowerControl {
            opcode,
            message: LowerControlMessage::Segmented {
                seq_zero,
                seg_o,
                seg_n,
                segment_m: Vec::from_slice(segment_m)
                    .map_err(|_| ParseError::InsufficientBuffer)?,
            },
        })
    }

    fn parse_unsegmented_access(data: &[u8]) -> Result<LowerAccess, ParseError> {
        let akf = data[0] & 0b01000000 != 0;
        let aid = data[0] & 0b00111111;
        Ok(LowerAccess {
            akf,
            aid: aid.into(),
            message: LowerAccessMessage::Unsegmented(
                Vec::from_slice(&data[1..]).map_err(|_| ParseError::InsufficientBuffer)?,
            ),
        })
    }

    fn parse_segmented_access(data: &[u8]) -> Result<LowerAccess, ParseError> {
        let akf = data[0] & 0b01000000 != 0;
        let aid = data[0] & 0b00111111;
        let szmic = SzMic::parse(data[1] & 0b10000000);
        let seq_zero = u16::from_be_bytes([data[1] & 0b01111111, data[2] & 0b11111100]) >> 2;
        let seg_o = (u16::from_be_bytes([data[2] & 0b00000011, data[3] & 0b11100000]) >> 5) as u8;
        let seg_n = data[3] & 0b00011111;
        let segment_m = &data[4..];

        Ok(LowerAccess {
            akf,
            aid: aid.into(),
            message: LowerAccessMessage::Segmented {
                szmic,
                seq_zero,
                seg_o,
                seg_n,
                segment_m: Vec::from_slice(&segment_m)
                    .map_err(|_| ParseError::InsufficientBuffer)?,
            },
        })
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        match self {
            LowerPDU::Access(inner) => inner.emit(xmit),
            LowerPDU::Control(inner) => inner.emit(xmit),
        }
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LowerAccess {
    pub(crate) akf: bool,
    pub(crate) aid: ApplicationKeyIdentifier,
    pub(crate) message: LowerAccessMessage,
}

impl LowerAccess {
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        let seg_akf_aid = match self.message {
            LowerAccessMessage::Unsegmented(_) => {
                if self.akf {
                    u8::from(self.aid) | 0b01000000
                } else {
                    u8::from(self.aid)
                }
            }
            LowerAccessMessage::Segmented { .. } => {
                if self.akf {
                    u8::from(self.aid) | 0b11000000
                } else {
                    u8::from(self.aid) | 0b10000000
                }
            }
        };
        xmit.push(seg_akf_aid).map_err(|_| InsufficientBuffer)?;
        self.message.emit(xmit)
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct LowerControl {
    pub(crate) opcode: Opcode,
    pub(crate) message: LowerControlMessage,
}

impl LowerControl {
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        xmit.push(self.opcode as u8)
            .map_err(|_| InsufficientBuffer)?;
        match &self.message {
            LowerControlMessage::Unsegmented { parameters } => {
                xmit.extend_from_slice(&parameters)
                    .map_err(|_| InsufficientBuffer)?;
            }
            LowerControlMessage::Segmented { .. } => {
                todo!("emit segmented lower control message");
            }
        }

        Ok(())
    }
}

#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SzMic {
    Bit32,
    Bit64,
}

impl SzMic {
    pub fn parse(data: u8) -> Self {
        if data != 0 {
            Self::Bit64
        } else {
            Self::Bit32
        }
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LowerAccessMessage {
    Unsegmented(Vec<u8, 15>),
    Segmented {
        szmic: SzMic,
        seq_zero: u16,
        seg_o: u8,
        seg_n: u8,
        segment_m: Vec<u8, 12>,
    },
}

impl LowerAccessMessage {
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        match self {
            LowerAccessMessage::Unsegmented(inner) => xmit
                .extend_from_slice(&inner)
                .map_err(|_| InsufficientBuffer),
            LowerAccessMessage::Segmented {
                szmic,
                seq_zero,
                seg_o,
                seg_n,
                segment_m,
            } => {
                let mut header = [0; 3];
                match szmic {
                    // small szmic + first 7 bits of seq_zero
                    SzMic::Bit32 => {
                        header[0] = 0b00000000 | ((seq_zero & 0b1111111000000) >> 6) as u8;
                    }
                    // big szmic + first 7 bits of seq_zero
                    SzMic::Bit64 => {
                        header[0] = 0b10000000 | ((seq_zero & 0b1111111000000) >> 6) as u8;
                    }
                }
                // last 6 bits of seq_zero + first 2 bits of seg_o
                header[1] = ((seq_zero & 0b111111) << 2) as u8 | ((seg_o & 0b00011000) >> 2) as u8;
                header[2] = ((seg_o & 0b00000111) << 5) | (seg_n & 0b00011111);
                xmit.extend_from_slice(&header)
                    .map_err(|_| InsufficientBuffer)?;
                xmit.extend_from_slice(segment_m)
                    .map_err(|_| InsufficientBuffer)?;
                Ok(())
            }
        }
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LowerControlMessage {
    Unsegmented {
        parameters: Vec<u8, 11>,
    },
    Segmented {
        seq_zero: u16,
        seg_o: u8,
        seg_n: u8,
        segment_m: Vec<u8, 8>,
    },
}

#[derive(Copy, Clone, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Opcode {
    SegmentedAcknowledgement = 0x00,
    FriendPoll = 0x01,
    FriendUpdate = 0x02,
    FriendRequest = 0x03,
    FriendOffer = 0x04,
    FriendClear = 0x05,
    FriendClearConfirm = 0x06,
    FriendSubscriptionListAdd = 0x07,
    FriendSubscriptionListRemove = 0x08,
    FriendSubscriptionListConfirm = 0x09,
    Heatbeat = 0x0A,
}

impl Opcode {
    pub fn parse(data: u8) -> Option<Opcode> {
        match data {
            0x00 => Some(Self::SegmentedAcknowledgement),
            0x01 => Some(Self::FriendPoll),
            0x02 => Some(Self::FriendUpdate),
            0x03 => Some(Self::FriendRequest),
            0x04 => Some(Self::FriendOffer),
            0x05 => Some(Self::FriendClear),
            0x06 => Some(Self::FriendClearConfirm),
            0x07 => Some(Self::FriendSubscriptionListAdd),
            0x08 => Some(Self::FriendSubscriptionListRemove),
            0x09 => Some(Self::FriendSubscriptionListConfirm),
            0x0A => Some(Self::Heatbeat),
            _ => None,
        }
    }
}

pub struct SegmentAck {
    seq_zero: u16,
    block_ack: u32,
}
