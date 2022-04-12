use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::driver::node::State;
use crate::drivers::ble::mesh::driver::pipeline::PipelineContext;
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::provisionable::Provisionable;
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::provisioning_bearer::{BearerMessage, ProvisioningBearer};
use crate::drivers::ble::mesh::generic_provisioning::Reason;
use crate::drivers::ble::mesh::pdu::bearer::advertising::AdvertisingPDU;
use crate::drivers::ble::mesh::provisioning::Capabilities;

pub mod provisionable;
pub mod provisioning_bearer;
pub mod segmentation;

pub(crate) struct UnprovisionedPipeline {
    provisioning_bearer: ProvisioningBearer,
    provisionable: Provisionable,
}

impl UnprovisionedPipeline {
    pub(crate) fn new(capabilities: Capabilities) -> Self {
        Self {
            provisioning_bearer: Default::default(),
            provisionable: Provisionable::new(capabilities),
        }
    }

    pub(crate) async fn process_inbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        pdu: &AdvertisingPDU,
    ) -> Result<Option<State>, DeviceError> {
        if let Some(message) = self.provisioning_bearer.process_inbound(ctx, pdu).await? {
            match message {
                BearerMessage::ProvisioningPDU(provisioning_pdu) => {
                    if let Some(outbound) = self
                        .provisionable
                        .process_inbound(ctx, provisioning_pdu)
                        .await?
                    {
                        for pdu in self.provisioning_bearer.process_outbound(outbound)? {
                            ctx.transmit_advertising_pdu(pdu).await?;
                        }
                    }
                    Ok(Some(State::Provisioning))
                }
                BearerMessage::Close(reason) => {
                    self.provisioning_bearer.reset();
                    self.provisionable.reset();
                    match reason {
                        Reason::Success => Ok(Some(State::Provisioned)),
                        _ => Ok(Some(State::Unprovisioned)),
                    }
                }
            }
        } else {
            Ok(None)
        }
    }

    pub(crate) async fn try_retransmit<C: PipelineContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        self.provisioning_bearer.try_retransmit(ctx).await
    }
}
