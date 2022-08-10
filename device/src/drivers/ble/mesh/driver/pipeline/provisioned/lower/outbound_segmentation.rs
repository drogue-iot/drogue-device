use crate::drivers::ble::mesh::driver::pipeline::provisioned::lower::{
    CleartextNetworkPDUSegments, LowerContext,
};
use crate::drivers::ble::mesh::driver::DeviceError;
use embassy_executor::time::{Duration, Instant};
use heapless::Vec;

struct Entry {
    ttl: u8,
    seq_zero: u16,
    segments: CleartextNetworkPDUSegments,
    deadline: Instant,
}

pub struct OutboundSegmentation<const N: usize = 3> {
    in_flight: Vec<Option<Entry>, N>,
}

impl<const N: usize> Default for OutboundSegmentation<N> {
    fn default() -> Self {
        let mut in_flight = Vec::new();
        for _ in 0..N {
            in_flight.push(None).ok();
        }
        Self { in_flight }
    }
}

impl<const N: usize> OutboundSegmentation<N> {
    pub fn register(
        &mut self,
        seq_zero: u16,
        ttl: u8,
        segments: CleartextNetworkPDUSegments,
    ) -> Result<(), DeviceError> {
        if let Some(entry) = self.in_flight.iter_mut().find(|e| matches!(e, None)) {
            *entry = Some(Entry {
                seq_zero,
                segments,
                ttl,
                deadline: Instant::now() + Duration::from_millis(200 + 50 * ttl as u64),
            });
            Ok(())
        } else {
            Err(DeviceError::InsufficientBuffer)
        }
    }

    pub fn ack(&mut self, seq_zero: u16, block_ack: u32) {
        if let Some(entry) = self.in_flight.iter_mut().find(|e| {
            if let Some(entry) = e {
                entry.seq_zero == seq_zero
            } else {
                false
            }
        }) {
            if let Some(inner) = entry {
                inner.deadline =
                    Instant::now() + Duration::from_millis(200 + 50 * inner.ttl as u64);
                if inner.segments.ack(block_ack) {
                    *entry = None;
                }
            }
        }
    }

    fn next_deadline(&self) -> Option<Instant> {
        let mut deadline = None;

        for entry in &self.in_flight {
            if let Some(inner) = entry {
                if let Some(inner_deadline) = deadline {
                    if inner.deadline < inner_deadline {
                        deadline.replace(inner.deadline);
                    }
                } else {
                    deadline.replace(inner.deadline);
                }
            }
        }

        deadline
    }

    pub fn retransmit<C: LowerContext>(
        &mut self,
        ctx: &C,
    ) -> Result<Option<CleartextNetworkPDUSegments<64>>, DeviceError> {
        let now = Instant::now();

        let mut segments = CleartextNetworkPDUSegments::new_empty();

        for e in self.in_flight.iter_mut() {
            if let Some(entry) = e {
                if entry.deadline < now {
                    for s in &entry.segments.segments {
                        if let Some(segment) = s {
                            info!("rxmt!");
                            segments.add(segment.clone())?;
                        }
                    }
                }
            }
        }

        ctx.ack_deadline(self.next_deadline());

        Ok(Some(segments))
    }
}
