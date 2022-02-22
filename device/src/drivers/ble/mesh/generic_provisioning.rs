use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::InsufficientBuffer;
use heapless::Vec;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GenericProvisioningPDU {
    TransactionStart(TransactionStart),
    TransactionAck,
    TransactionContinuation(TransactionContinuation),
    ProvisioningBearerControl(ProvisioningBearerControl),
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TransactionStart {
    pub seg_n: u8,
    pub total_len: u16,
    pub fcs: u8,
    pub data: Vec<u8, 64>,
}

impl TransactionStart {
    pub fn parse(data: &[u8]) -> Result<Self, GenericProvisioningError> {
        if data.len() >= 5 {
            let seg_n = (data[0] & 0b11111100) >> 2;
            let total_len = u16::from_be_bytes([data[1], data[2]]);
            let fcs = data[3];
            Ok(Self {
                seg_n,
                total_len,
                fcs,
                data: Vec::from_slice(&data[4..])
                    .map_err(|_| GenericProvisioningError::InvalidSize)?,
            })
        } else {
            Err(GenericProvisioningError::InvalidSize)
        }
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        xmit.push(self.seg_n << 2).map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.total_len.to_be_bytes())
            .map_err(|_| InsufficientBuffer)?;
        xmit.push(self.fcs).map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&*self.data)
            .map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TransactionContinuation {
    pub segment_index: u8,
    pub data: Vec<u8, 64>,
}

impl TransactionContinuation {
    pub fn parse(data: &[u8]) -> Result<Self, GenericProvisioningError> {
        if data.len() >= 2 {
            let segment_index = (data[0] & 0b11111100) >> 2;
            Ok(Self {
                segment_index,
                data: Vec::from_slice(&data[1..])
                    .map_err(|_| GenericProvisioningError::InvalidSize)?,
            })
        } else {
            Err(GenericProvisioningError::InvalidSize)
        }
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        xmit.push(self.segment_index << 2 | 0b10)
            .map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&*self.data)
            .map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum GenericProvisioningError {
    InvalidSize,
    InvalidGpcf,
    InvalidBits,
    InvalidReason,
    Other,
}

impl GenericProvisioningPDU {
    pub fn parse(data: &[u8]) -> Result<Self, GenericProvisioningError> {
        if data.len() >= 1 {
            match data[0] & 0b11 {
                0b00 => Ok(Self::TransactionStart(TransactionStart::parse(data)?)),
                0b01 => Self::parse_transaction_ack(data),
                0b10 => Ok(Self::TransactionContinuation(
                    TransactionContinuation::parse(data)?,
                )),
                0b11 => Ok(Self::ProvisioningBearerControl(
                    ProvisioningBearerControl::parse(data)?,
                )),
                _ => Err(GenericProvisioningError::InvalidGpcf),
            }
        } else {
            Err(GenericProvisioningError::InvalidSize)
        }
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        match self {
            GenericProvisioningPDU::TransactionStart(tx_start) => {
                tx_start.emit(xmit)?;
            }
            GenericProvisioningPDU::TransactionAck => {
                // Ack is simple.
                xmit.push(0b00000001).map_err(|_| InsufficientBuffer)?;
            }
            GenericProvisioningPDU::TransactionContinuation(tx_cont) => {
                tx_cont.emit(xmit)?;
            }
            GenericProvisioningPDU::ProvisioningBearerControl(pbc) => {
                pbc.emit(xmit)?;
            }
        }

        Ok(())
    }

    fn parse_transaction_ack(data: &[u8]) -> Result<Self, GenericProvisioningError> {
        if data.len() == 1 {
            if data[0] & 0b11111100 == 0 {
                Ok(Self::TransactionAck)
            } else {
                Err(GenericProvisioningError::InvalidBits)
            }
        } else {
            Err(GenericProvisioningError::InvalidSize)
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ProvisioningBearerControl {
    LinkOpen(Uuid),
    LinkAck,
    LinkClose(Reason),
}

impl ProvisioningBearerControl {
    pub fn parse(data: &[u8]) -> Result<Self, GenericProvisioningError> {
        match (data[0] & 0b111111) >> 2 {
            0x00 => Self::parse_link_open(&data[1..]),
            0x01 => Self::parse_link_ack(&data[1..]),
            0x02 => Self::parse_link_close(&data[1..]),
            _ => Err(GenericProvisioningError::InvalidGpcf),
        }
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        match self {
            ProvisioningBearerControl::LinkOpen(_) => {}
            ProvisioningBearerControl::LinkAck => {
                xmit.push(0x01 << 2 | 0b11)
                    .map_err(|_| InsufficientBuffer)?;
            }
            ProvisioningBearerControl::LinkClose(_) => {}
        }

        Ok(())
    }

    fn parse_link_open(data: &[u8]) -> Result<Self, GenericProvisioningError> {
        if data.len() == 16 {
            let uuid = Uuid([
                data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8],
                data[9], data[10], data[11], data[12], data[13], data[14], data[15],
            ]);
            Ok(Self::LinkOpen(uuid))
        } else {
            Err(GenericProvisioningError::InvalidSize)
        }
    }

    fn parse_link_ack(data: &[u8]) -> Result<Self, GenericProvisioningError> {
        if data.len() == 0 {
            Ok(Self::LinkAck)
        } else {
            Err(GenericProvisioningError::InvalidSize)
        }
    }

    fn parse_link_close(data: &[u8]) -> Result<Self, GenericProvisioningError> {
        if data.len() == 1 {
            Ok(Self::LinkClose(Reason::parse(data[0])?))
        } else {
            Err(GenericProvisioningError::InvalidSize)
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Reason {
    Success = 0x00,
    Timeout = 0x01,
    Fail = 0x02,
}

impl Reason {
    pub fn parse(reason: u8) -> Result<Self, GenericProvisioningError> {
        match reason {
            0x00 => Ok(Self::Success),
            0x01 => Ok(Self::Timeout),
            0x02 => Ok(Self::Fail),
            _ => Err(GenericProvisioningError::InvalidReason),
        }
    }
}
