use crate::drivers::ble::mesh::pdu::network::{NetworkPDU, ObfuscatedAndEncryptedNetworkPDU};
use embassy::time::{Duration, Instant, Timer};

use crate::drivers::ble::mesh::driver::pipeline::mesh::MeshContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use heapless::Vec;

pub(crate) struct Item {
    pdu: ObfuscatedAndEncryptedNetworkPDU,
    count: u8,
    interval: Duration,
    last: Option<Instant>,
}

impl Item {
    fn is_ready(&self, now: Instant) -> bool {
        if let Some(last) = self.last {
            now.duration_since(last) > self.interval
        } else {
            true
        }
    }

    fn next_deadline(&self, now: Instant) -> Instant {
        now + self.interval
    }
}

pub(crate) struct TransmitQueue<const N: usize = 15> {
    items: Vec<Option<Item>, N>,
    interval: Duration,
    count: u8,
}

impl<const N: usize> TransmitQueue<N> {
    pub(crate) fn push(&mut self, pdu: ObfuscatedAndEncryptedNetworkPDU, count: u8, interval: Duration) {
        if let Some(slot) = self.items.iter_mut().find(|e| matches!(e, None)) {
            slot.replace(Item {
                pdu,
                count,
                interval,,
                last: None,
            });
        } /* else find one to purge? */
    }

    fn next_deadline(&self, now: Instant) -> Option<Duration> {
        let mut next_deadline = None;

        for item in self.items {
            match (next_deadline, item) {
                (Some(next), Some(item)) => {
                    let item_next_deadline = item.next_deadline(now);
                    if item_next_deadline < next {
                        next_deadline.replace(item_next_deadline)
                    }
                }
                (None, Some(item)) => {
                    let item_next_deadline = item.next_deadline(now);
                    next_deadline.replace(item_next_deadline)
                }
                _ => {
                    // nothing
                }
            }
        }

        if let Some(next_deadline) = next_deadline {
            Some(next_deadline.duration_since(now))
        } else {
            None
        }
    }

    fn rectify_with_transmit_limits(&self, deadline: Duration) -> Duration {
        if deadline.as_millis() < self.interval {
            self.interval
        } else {
            deadline
        }
    }

    pub(crate) async fn run<C: MeshContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        let now = Instant::now();
        if let Some(next_deadline) = self.next_deadline(now) {
            let next_deadline = self.rectify_with_transmit_limits(next_deadline);
            Timer::after(next_deadline).await;
            // in a given transmit run, we can transmit up to count PDUs.
            for _ in 0..self.count {
                if ! self.transmit_next(ctx)? {
                    // we exhausted ourselves
                    return Ok(())
                }
            }
            Ok(())
        } else {
            // no deadline
            Ok(())
        }
    }

    pub(crate) async fn transmit_next<C: MeshContext>(
        &mut self,
        ctx: &C,
    ) -> Result<bool, DeviceError> {
        // first, find any that haven't been transmitted at all
        if self.transmit_untransmitted(ctx).await? {
            return Ok(true);
        }

        if self.transmit_ready(ctx).await? {
            return Ok(true)
        }

        Ok(false)
    }

    async fn transmit_untransmitted<C: MeshContext>(
        &mut self,
        ctx: &C,
    ) -> Result<bool, DeviceError> {
        if let Some(Some(item)) = self.items.iter_mut().find(|e| {
            if let Some(e) = e {
                matches!(e.last, None)
            } else {
                false
            }
        }) {
            ctx.transmit_mesh_pdu(&item.pdu).await?;
            if item.count > 1 {
                item.last.replace(Instant::now());
                item.count = item.count - 1;
            }
        }

        Ok(true)
    }

    async fn transmit_ready<C: MeshContext>(&mut self, ctx: &C) -> Result<bool, DeviceError> {
        let now = Instant::now();
        if let Some(item) = self.items.iter_mut().find(|e| {
            if let Some(e) = e {
                e.is_ready(now)
            } else {
                false
            }
        }) {
            if let Some(inner) = item {
                ctx.transmit_mesh_pdu(&inner.pdu).await?;
                if inner.count > 1 {
                    inner.last.replace(now);
                    inner.count = inner.count - 1;
                } else {
                    item.take();
                }
            }
        }

        Ok(true)
    }
}
