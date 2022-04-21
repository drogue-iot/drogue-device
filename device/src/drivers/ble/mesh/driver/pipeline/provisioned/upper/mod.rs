use embassy::time::Instant;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use crate::drivers::ble::mesh::pdu::upper::{UpperAccess, UpperPDU};

use heapless::Vec;
use crate::drivers::ble::mesh::driver::pipeline::mesh::{NetworkRetransmitDetails, PublishRetransmitDetails};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::transmit::ModelKey;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::upper::publish::Publish;

pub mod publish;

pub trait UpperContext {
    fn publish_deadline(&self, deadline: Option<Instant>);
}

pub struct Upper {
    publish: Publish,
}

impl Default for Upper {
    fn default() -> Self {
        Self {
            publish: Default::default(),
        }
    }
}

impl Upper {
    pub fn process_inbound<C: UpperContext>(
        &mut self,
        _ctx: &C,
        pdu: UpperPDU,
    ) -> Result<Option<AccessMessage>, DeviceError> {
        // todo: split access and control handling, wrap with an enum, I guess.
        match pdu {
            UpperPDU::Control(_control) => {
                todo!("inbound upper pdu control")
            }
            UpperPDU::Access(access) => {
                let message = AccessMessage::parse(&access)?;
                Ok(Some(message))
            }
        }
    }

    pub fn process_outbound<C: UpperContext>(
        &mut self,
        _ctx: &C,
        message: &AccessMessage,
        publish: Option<(ModelKey, PublishRetransmitDetails)>,
    ) -> Result<Option<UpperPDU>, DeviceError> {
        // todo: split access and control handling, wrap with an enum, I guess.
        self.publish.process_outbound(message, publish);

        let mut payload = Vec::new();
        message.emit(&mut payload)?;
        Ok(Some(UpperPDU::Access(UpperAccess {
            ttl: message.ttl,
            network_key: message.network_key,
            ivi: message.ivi,
            nid: message.nid,
            akf: message.akf,
            aid: message.aid,
            src: message.src,
            dst: message.dst,
            payload,
        })))
    }
}
