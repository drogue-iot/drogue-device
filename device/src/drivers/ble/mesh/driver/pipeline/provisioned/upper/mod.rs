use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use crate::drivers::ble::mesh::pdu::upper::{UpperAccess, UpperPDU};

use heapless::Vec;

pub trait UpperContext {}

pub struct Upper {}

impl Default for Upper {
    fn default() -> Self {
        Self {}
    }
}

impl Upper {
    pub async fn process_inbound<C: UpperContext>(
        &mut self,
        _ctx: &C,
        pdu: UpperPDU,
    ) -> Result<Option<AccessMessage>, DeviceError> {
        // todo: split access and control handling, wrap with an enum, I guess.
        match pdu {
            UpperPDU::Control(_control) => {
                todo!()
            }
            UpperPDU::Access(access) => {
                let message = AccessMessage::parse(&access)?;
                Ok(Some(message))
            }
        }
    }

    pub async fn process_outbound<C: UpperContext>(
        &mut self,
        _ctx: &C,
        message: AccessMessage,
    ) -> Result<Option<UpperPDU>, DeviceError> {
        // todo: split access and control handling, wrap with an enum, I guess.
        let mut payload = Vec::new();
        message.emit(&mut payload)?;
        Ok(Some(UpperPDU::Access(UpperAccess {
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
