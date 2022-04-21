use crate::drivers::ble::mesh::pdu::network::{NetworkPDU, ObfuscatedAndEncryptedNetworkPDU};
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use embassy::blocking_mutex::raw::ThreadModeRawMutex;
use embassy::channel::Channel;
use embassy::time::{Duration, Instant, Timer};
use futures::future::{select, Either};
use futures::{pin_mut, FutureExt};

use crate::drivers::ble::mesh::address::UnicastAddress;
use crate::drivers::ble::mesh::driver::pipeline::mesh::{MeshContext, NetworkRetransmitDetails};
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::ModelIdentifier;
use heapless::Vec;

#[derive(Copy, Clone, PartialEq)]
pub struct ModelKey(UnicastAddress, ModelIdentifier);

impl ModelKey {
    pub fn new(addr: UnicastAddress, model_id: ModelIdentifier) -> Self {
        Self(addr, model_id)
    }
}

#[derive(Copy, Clone)]
pub(crate) struct Correlation {
    model_key: ModelKey,
    seq_zero: u16,
}

impl Correlation {
    pub fn new(seq_zero: u16, model_key: Option<ModelKey>) -> Option<Self> {
        match model_key {
            None => None,
            Some(model_key) => Some(Correlation {
                seq_zero,
                model_key,
            }),
        }
    }
}

pub(crate) struct Item {
    pdu: ObfuscatedAndEncryptedNetworkPDU,
    count: u8,
    interval: Duration,
    last: Option<Instant>,
    correlation: Option<Correlation>,
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
        if let Some(last) = self.last {
            last + self.interval
        } else {
            now
        }
    }
}

pub(crate) struct Transmit<const N: usize = 15> {
    items: RefCell<Vec<Option<Item>, N>>,
    interval: Duration,
    count: u8,
}

impl<const N: usize> Default for Transmit<N> {
    fn default() -> Self {
        Self::new()
    }
}

/// The network transmit queue will retransmit, based upon configuration,
/// network PDUs, unmodified. The *same* sequence number will be used
/// for each retransmit.
///
/// Correlation is provided to allow for "old" publish messages to be purged
/// in the case of a new publish arriving while a previous value is still being
/// transmitted/retransmitted.
impl<const N: usize> Transmit<N> {
    pub(crate) fn new() -> Self {
        Self {
            items: RefCell::new(Default::default()),
            interval: Duration::from_millis(20),
            count: 2,
        }
    }

    pub(crate) async fn process_outbound<C: MeshContext>(
        &self,
        ctx: &C,
        pdu: ObfuscatedAndEncryptedNetworkPDU,
        correlation: Option<Correlation>,
        network_retransmit: &NetworkRetransmitDetails,
    ) -> Result<(), DeviceError> {
        // At least transmit once on the network
        ctx.transmit_mesh_pdu(&pdu).await?;

        /// then look for a place to hang onto it for retransmits
        if let Some(slot) = self
            .items
            .borrow_mut()
            .iter_mut()
            .find(|e| matches!(e, None))
        {
            slot.replace(Item {
                pdu,
                count: network_retransmit.count,
                last: None,
                interval: network_retransmit.interval,
                correlation,
            });
        } /* else find one to purge? */

        // remove any previous correlations
        if let Some(new_correlation) = &correlation {
            self.items
                .borrow_mut()
                .iter_mut()
                .filter(|e| {
                    if let Some(inner) = e {
                        if let Some(correlation) = &inner.correlation {
                            if correlation.model_key == new_correlation.model_key {
                                return correlation.seq_zero != new_correlation.seq_zero;
                            }
                        }
                    }
                    return false;
                })
                .for_each(|e| {
                    e.take();
                });
        }

        Ok(())
    }

    fn next_deadline(&self, now: Instant) -> Option<Duration> {
        let mut next_deadline = None;

        for item in self.items.borrow().iter() {
            match (next_deadline, item) {
                (Some(next), Some(item)) => {
                    let item_next_deadline = item.next_deadline(now);
                    if item_next_deadline < next {
                        next_deadline.replace(item_next_deadline);
                    }
                }
                (None, Some(item)) => {
                    let item_next_deadline = item.next_deadline(now);
                    next_deadline.replace(item_next_deadline);
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

    async fn transmit_untransmitted<C: MeshContext>(&self, ctx: &C) -> Result<bool, DeviceError> {
        for each in self.items.borrow_mut().iter_mut().filter(|e| {
            if let Some(e) = e {
                matches!(e.last, None)
            } else {
                false
            }
        }) {
            if let Some(inner) = each {
                ctx.transmit_mesh_pdu(&inner.pdu).await?;
                if inner.count > 1 {
                    inner.last.replace(Instant::now());
                    inner.count -= 1;
                }
            }
        }

        Ok(true)
    }

    async fn transmit_ready<C: MeshContext>(&self, ctx: &C) -> Result<bool, DeviceError> {
        let now = Instant::now();
        if let Some(item) = self.items.borrow_mut().iter_mut().find(|e| {
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
                    inner.count -= 1;
                } else {
                    item.take();
                }
            }
        }

        Ok(true)
    }
}
