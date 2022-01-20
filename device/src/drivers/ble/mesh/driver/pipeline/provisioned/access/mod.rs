use crate::drivers::ble::mesh::address::UnicastAddress;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::access::beacon::Beacon;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::pdu::access::{AccessMessage, AccessPayload, Config, Health};
use futures::TryFutureExt;

pub mod beacon;

pub trait AccessContext {
    fn primary_unicast_address(&self) -> Option<UnicastAddress>;
}

pub struct Access {
    beacon: Beacon,
}

impl Default for Access {
    fn default() -> Self {
        Self {
            beacon: Default::default(),
        }
    }
}

impl Access {
    pub async fn process_inbound<C: AccessContext>(
        &mut self,
        ctx: &C,
        message: AccessMessage,
    ) -> Result<Option<AccessMessage>, DeviceError> {
        Ok(self
            .beacon
            .process_inbound(ctx, &message)
            .await?
            .or_else(|| None))
    }
}
