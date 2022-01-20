use crate::drivers::ble::mesh::driver::pipeline::segmentation::fcs;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::generic_provisioning::{
    GenericProvisioningPDU, TransactionContinuation, TransactionStart,
};
use crate::drivers::ble::mesh::provisioning::ProvisioningPDU;
use heapless::Vec;

const TRANSACTION_START_MTU: usize = 20;
const TRANSACTION_CONTINUATION_MTU: usize = 23;

pub struct OutboundSegments {
    pdu: Vec<u8, 128>,
    num_segments: u8,
    fcs: u8,
}

impl OutboundSegments {
    pub fn new(pdu: ProvisioningPDU) -> Result<Self, DeviceError> {
        let mut data = Vec::new();
        pdu.emit(&mut data)?;
        let fcs = fcs(&data);
        let num_segments = Self::num_chunks(&data);

        Ok(Self {
            pdu: data,
            num_segments,
            fcs: fcs,
        })
    }

    pub fn iter(&self) -> OutboundSegmentsIter {
        OutboundSegmentsIter::new(self)
    }

    fn num_chunks(pdu: &[u8]) -> u8 {
        let mut len = pdu.len();
        // TransactionStart can hold 20
        if len <= TRANSACTION_START_MTU {
            return 1;
        }
        let mut num_chunks = 1;
        len = len - TRANSACTION_START_MTU;
        // TransactionContinuation can hold 24
        while len > 0 {
            num_chunks = num_chunks + 1;
            if len > TRANSACTION_CONTINUATION_MTU {
                len = len - TRANSACTION_CONTINUATION_MTU;
            } else {
                break;
            }
        }

        num_chunks
    }
}

pub struct OutboundSegmentsIter<'a> {
    segments: &'a OutboundSegments,
    cur: usize,
}

impl<'a> OutboundSegmentsIter<'a> {
    fn new(segments: &'a OutboundSegments) -> Self {
        Self { segments, cur: 0 }
    }
}

impl<'a> Iterator for OutboundSegmentsIter<'a> {
    type Item = GenericProvisioningPDU;

    fn next(&mut self) -> Option<Self::Item> {
        let cur = self.cur;
        self.cur = self.cur + 1;

        if cur == 0 {
            let chunk = if self.segments.pdu.len() <= TRANSACTION_START_MTU {
                &self.segments.pdu
            } else {
                &self.segments.pdu[0..TRANSACTION_START_MTU]
            };

            Some(GenericProvisioningPDU::TransactionStart(TransactionStart {
                seg_n: self.segments.num_segments as u8 - 1,
                total_len: self.segments.pdu.len() as u16,
                fcs: self.segments.fcs,
                data: Vec::from_slice(chunk).ok()?,
            }))
        } else {
            let chunk_start = TRANSACTION_START_MTU + ((cur - 1) * TRANSACTION_CONTINUATION_MTU);
            if chunk_start >= self.segments.pdu.len() {
                None
            } else {
                let chunk_end = chunk_start + TRANSACTION_CONTINUATION_MTU;
                let chunk = if chunk_end <= self.segments.pdu.len() {
                    &self.segments.pdu[chunk_start..chunk_end]
                } else {
                    &self.segments.pdu[chunk_start..]
                };
                Some(GenericProvisioningPDU::TransactionContinuation(
                    TransactionContinuation {
                        segment_index: cur as u8,
                        data: Vec::from_slice(chunk).ok()?,
                    },
                ))
            }
        }
    }
}
