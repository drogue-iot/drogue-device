use crate::drivers::ble::mesh::driver::pipeline::mesh::PublishRetransmitDetails;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::transmit::ModelKey;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use embassy_time::Instant;

use crate::drivers::ble::mesh::driver::node::outbound::OutboundPublishMessage;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::upper::UpperContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use heapless::Vec;

pub struct Entry {
    message: OutboundPublishMessage,
    retransmit: PublishRetransmitDetails,
    last: Instant,
}

pub struct Publish<const N: usize = 3> {
    cache: Vec<Option<Entry>, N>,
}

impl<const N: usize> Default for Publish<N> {
    fn default() -> Self {
        let mut cache = Vec::new();
        for _ in 0..N {
            cache.push(None).ok();
        }
        Self { cache }
    }
}

impl<const N: usize> Publish<N> {
    fn next_deadline(&self) -> Option<Instant> {
        let mut deadline = None;

        for entry in self.cache.iter() {
            if let Some(inner) = entry {
                let candidate = inner.last + inner.retransmit.interval;

                if let Some(inner_deadline) = deadline {
                    if inner_deadline > candidate {
                        deadline.replace(candidate);
                    }
                } else {
                    deadline.replace(candidate);
                }
            }
        }

        deadline
    }

    pub fn process_outbound(
        &mut self,
        message: &AccessMessage,
        publish: Option<(ModelKey, PublishRetransmitDetails)>,
    ) {
        if let Some(publish) = publish {
            if let Some(prev) = self.cache.iter_mut().find(|e| {
                if let Some(inner) = e {
                    inner.message.model_identifier == publish.0.model_identifier()
                        && inner.message.element_address == publish.0.unicast_address()
                } else {
                    false
                }
            }) {
                prev.replace(Entry {
                    message: OutboundPublishMessage {
                        element_address: publish.0.unicast_address(),
                        model_identifier: publish.0.model_identifier(),
                        payload: message.payload.clone(),
                    }
                    .clone(),
                    retransmit: publish.1,
                    last: Instant::now(),
                });
            } else {
                if let Some(empty) = self.cache.iter_mut().find(|e| matches!(e, None)) {
                    empty.replace(Entry {
                        message: OutboundPublishMessage {
                            element_address: publish.0.unicast_address(),
                            model_identifier: publish.0.model_identifier(),
                            payload: message.payload.clone(),
                        },
                        retransmit: publish.1,
                        last: Instant::now(),
                    });
                }
            }
        }
    }

    pub async fn retransmit<C: UpperContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        let now = Instant::now();

        for entry in self.cache.iter_mut() {
            if let Some(ref mut inner) = entry {
                if inner.last + inner.retransmit.interval >= now {
                    ctx.republish(inner.message.clone()).await;
                    inner.retransmit.count -= 1;
                    if inner.retransmit.count == 0 {
                        entry.take();
                    } else {
                        inner.last = now;
                    }
                }
            }
        }

        ctx.publish_deadline(self.next_deadline());

        Ok(())
    }
}
