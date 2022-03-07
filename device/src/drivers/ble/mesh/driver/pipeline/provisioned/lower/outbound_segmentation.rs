use crate::drivers::ble::mesh::driver::pipeline::provisioned::lower::CleartextNetworkPDUSegments;
use crate::drivers::ble::mesh::driver::DeviceError;
use embassy::time::{Duration, Instant};

struct Entry {
    seq_zero: u16,
    segments: CleartextNetworkPDUSegments,
    ts: Instant,
}

static mut IN_FLIGHT: [Option<Entry>; 3] = [None, None, None];

pub struct OutboundSegmentation {}

impl Default for OutboundSegmentation {
    fn default() -> Self {
        Self {}
    }
}

impl OutboundSegmentation {
    pub fn register(
        &mut self,
        seq_zero: u16,
        segments: CleartextNetworkPDUSegments,
    ) -> Result<(), DeviceError> {
        unsafe {
            if let Some(entry) = IN_FLIGHT.iter_mut().find(|e| matches!(e, None)) {
                *entry = Some(Entry {
                    seq_zero,
                    segments,
                    ts: Instant::now(),
                });
                Ok(())
            } else {
                Err(DeviceError::InsufficientBuffer)
            }
        }
    }

    pub fn ack(&mut self, seq_zero: u16, block_ack: u32) {
        unsafe {
            if let Some(entry) = IN_FLIGHT.iter_mut().find(|e| {
                if let Some(entry) = e {
                    entry.seq_zero == seq_zero
                } else {
                    false
                }
            }) {
                if let Some(inner) = entry {
                    inner.ts = Instant::now();
                    if inner.segments.ack(block_ack) {
                        *entry = None;
                    }
                }
            }
        }
    }

    pub fn process_retransmits(
        &mut self,
    ) -> Result<Option<CleartextNetworkPDUSegments<64>>, DeviceError> {
        let now = Instant::now();

        let mut segments = CleartextNetworkPDUSegments::new_empty();

        unsafe {
            for e in IN_FLIGHT.iter_mut() {
                if let Some(entry) = e {
                    if now.duration_since(entry.ts) > Duration::from_secs(7) {
                        *e = None
                    } else {
                        for s in &entry.segments.segments {
                            if let Some(segment) = s {
                                info!("rxmt!");
                                segments.add(segment.clone())?;
                            }
                        }
                    }
                }
            }
        }

        Ok(Some(segments))
    }
}
