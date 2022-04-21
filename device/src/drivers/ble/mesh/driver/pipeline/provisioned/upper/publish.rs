use crate::drivers::ble::mesh::driver::pipeline::mesh::{NetworkRetransmitDetails, PublishRetransmitDetails};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::transmit::ModelKey;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;

use heapless::Vec;

pub struct Entry {
    model_key: ModelKey,
    message: AccessMessage,
    retransmit: PublishRetransmitDetails,
}

pub struct Publish {
    cache: Vec<Option<Entry>, 15>,
}

impl Default for Publish {
    fn default() -> Self {
        Self {
            cache: Default::default(),
        }
    }
}

impl Publish {
    pub fn process_outbound(
        &mut self,
        message: &AccessMessage,
        publish: Option<(ModelKey, PublishRetransmitDetails)>,
    ) {
        if let Some(publish) = publish {
            if let Some(prev) = self.cache.iter_mut().find(|e| if let Some(inner) = e {
                inner.model_key == publish.0
            } else {
                false
            }) {
                prev.replace( Entry {
                    model_key: publish.0,
                    message: message.clone(),
                    retransmit: publish.1,
                });
            } else {
                if let Some(empty) = self.cache.iter_mut().find(|e| matches!(e, None)) {
                    empty.replace( Entry {
                        model_key: publish.0,
                        message: message.clone(),
                        retransmit: publish.1,
                    });
                }
            }
        }
    }
}
