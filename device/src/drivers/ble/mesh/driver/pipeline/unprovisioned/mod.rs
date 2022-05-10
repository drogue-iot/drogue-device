use crate::drivers::ble::mesh::driver::node::State;
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::provisionable::Provisionable;
use crate::drivers::ble::mesh::driver::pipeline::PipelineContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::interface::PDU;
use crate::drivers::ble::mesh::provisioning::{Capabilities, ProvisioningPDU};

pub mod provisionable;

pub(crate) struct UnprovisionedPipeline {
    provisionable: Provisionable,
}

impl UnprovisionedPipeline {
    pub(crate) fn new(capabilities: Capabilities) -> Self {
        Self {
            provisionable: Provisionable::new(capabilities),
        }
    }

    pub(crate) async fn process_inbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        pdu: ProvisioningPDU,
    ) -> Result<Option<State>, DeviceError> {
        if let Some(response) = self.provisionable.process_inbound(ctx, pdu).await? {
            ctx.transmit(&PDU::Provisioning(response)).await?;
            Ok(Some(State::Provisioning))
        } else {
            Ok(None)
        }
    }
}
