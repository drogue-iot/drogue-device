use crate::drivers::ble::mesh::pdu::network::ObfuscatedAndEncryptedNetworkPDU;
use embassy_time::{Duration, Instant};

use crate::drivers::ble::mesh::address::UnicastAddress;
use crate::drivers::ble::mesh::driver::pipeline::mesh::NetworkRetransmitDetails;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::NetworkContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::interface::PDU;
use crate::drivers::ble::mesh::model::ModelIdentifier;
use heapless::Vec;

#[derive(Copy, Clone, PartialEq)]
pub struct ModelKey(UnicastAddress, ModelIdentifier);

impl ModelKey {
    pub fn new(addr: UnicastAddress, model_id: ModelIdentifier) -> Self {
        Self(addr, model_id)
    }

    pub fn unicast_address(&self) -> UnicastAddress {
        self.0
    }

    pub fn model_identifier(&self) -> ModelIdentifier {
        self.1
    }
}

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
        if let Some(last) = self.last {
            last + self.interval
        } else {
            now
        }
    }
}

pub(crate) struct Transmit<const N: usize = 3> {
    items: Vec<Option<Item>, N>,
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
impl<const N: usize> Transmit<N> {
    pub(crate) fn new() -> Self {
        let mut items = Vec::new();
        for _ in 0..N {
            items.push(None).ok();
        }
        Self {
            items,
            interval: Duration::from_millis(20),
            count: 2,
        }
    }

    pub(crate) async fn process_outbound<C: NetworkContext>(
        &mut self,
        ctx: &C,
        pdu: ObfuscatedAndEncryptedNetworkPDU,
        network_retransmit: &NetworkRetransmitDetails,
    ) -> Result<(), DeviceError> {
        // At least transmit once on the network
        //ctx.transmit_mesh_pdu(&pdu).await?;
        ctx.transmit(&PDU::Network(pdu.clone())).await?;

        // then look for a place to hang onto it for retransmits
        if let Some(slot) = self.items.iter_mut().find(|e| matches!(*e, None)) {
            slot.replace(Item {
                pdu,
                count: network_retransmit.count,
                last: None,
                interval: network_retransmit.interval,
            });
        }

        let next_deadline = self.next_deadline(Instant::now());
        ctx.network_deadline(next_deadline);

        Ok(())
    }

    fn next_deadline(&self, now: Instant) -> Option<Instant> {
        let mut next_deadline = None;

        for item in self.items.iter() {
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

        next_deadline
    }

    pub(crate) async fn retransmit<C: NetworkContext>(
        &mut self,
        ctx: &C,
    ) -> Result<(), DeviceError> {
        self.transmit_untransmitted(ctx).await?;
        self.transmit_ready(ctx).await?;
        let next_deadline = self.next_deadline(Instant::now());
        ctx.network_deadline(next_deadline);
        Ok(())
    }

    async fn transmit_untransmitted<C: NetworkContext>(
        &mut self,
        ctx: &C,
    ) -> Result<bool, DeviceError> {
        for each in self.items.iter_mut().filter(|e| {
            if let Some(e) = e {
                matches!(e.last, None)
            } else {
                false
            }
        }) {
            if let Some(inner) = each {
                ctx.transmit(&PDU::Network(inner.pdu.clone())).await?;
                if inner.count > 1 {
                    inner.last.replace(Instant::now());
                    inner.count -= 1;
                }
            }
        }

        Ok(true)
    }

    async fn transmit_ready<C: NetworkContext>(&mut self, ctx: &C) -> Result<bool, DeviceError> {
        let now = Instant::now();
        if let Some(item) = self.items.iter_mut().find(|e| {
            if let Some(e) = e {
                e.is_ready(now)
            } else {
                false
            }
        }) {
            if let Some(inner) = item {
                ctx.transmit(&PDU::Network(inner.pdu.clone())).await?;
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
