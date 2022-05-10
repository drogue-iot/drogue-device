use crate::drivers::ble::mesh::driver::node::deadline::Expiration;
use crate::drivers::ble::mesh::driver::node::State;
use crate::drivers::ble::mesh::driver::pipeline::mesh::{
    NetworkRetransmitDetails, PublishRetransmitDetails,
};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::transmit::ModelKey;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::{
    ProvisionedContext, ProvisionedPipeline,
};
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::provisionable::UnprovisionedContext;
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::UnprovisionedPipeline;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::interface::PDU;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use crate::drivers::ble::mesh::provisioning::Capabilities;

pub mod mesh;
pub mod provisioned;
pub mod unprovisioned;

pub trait PipelineContext: UnprovisionedContext + ProvisionedContext {}

pub struct Pipeline {
    capabilities: Capabilities,
    inner: PipelineInner,
}

enum PipelineInner {
    Unconfigured,
    Unprovisioned(UnprovisionedPipeline),
    Provisioned(ProvisionedPipeline),
}

impl PipelineInner {
    async fn process_inbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        mut message: PDU,
    ) -> Result<Option<State>, DeviceError> {
        match self {
            PipelineInner::Unconfigured => Ok(None),
            PipelineInner::Unprovisioned(inner) => {
                if let PDU::Provisioning(pdu) = message {
                    inner.process_inbound(ctx, pdu).await
                } else {
                    Ok(None)
                }
            }
            PipelineInner::Provisioned(inner) => {
                if let PDU::Network(ref mut pdu) = message {
                    inner.process_inbound(ctx, pdu).await
                } else {
                    Ok(None)
                }
            }
        }
    }

    async fn process_outbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        message: &AccessMessage,
        publish: Option<(ModelKey, PublishRetransmitDetails)>,
        network_retransmit: NetworkRetransmitDetails,
    ) -> Result<(), DeviceError> {
        match self {
            PipelineInner::Unconfigured => Err(DeviceError::NotProvisioned),
            PipelineInner::Unprovisioned(_) => Err(DeviceError::NotProvisioned),
            PipelineInner::Provisioned(inner) => {
                inner
                    .process_outbound(ctx, message, publish, network_retransmit)
                    .await
            }
        }
    }

    pub async fn retransmit<C: PipelineContext>(
        &mut self,
        ctx: &C,
        expiration: Expiration,
    ) -> Result<(), DeviceError> {
        match self {
            PipelineInner::Unconfigured => Err(DeviceError::PipelineNotConfigured),
            PipelineInner::Unprovisioned(_) => Ok(()),
            PipelineInner::Provisioned(inner) => inner.retransmit(ctx, expiration).await,
        }
    }
}

impl Pipeline {
    pub fn new(capabilities: Capabilities) -> Self {
        Self {
            capabilities,
            inner: PipelineInner::Unconfigured,
        }
    }

    pub(crate) fn state(&mut self, state: State) {
        match state {
            State::Unprovisioned => {
                if !matches!(self.inner, PipelineInner::Unprovisioned(_)) {
                    self.inner = PipelineInner::Unprovisioned(UnprovisionedPipeline::new(
                        self.capabilities.clone(),
                    ))
                }
            }
            State::Provisioned => {
                if !matches!(self.inner, PipelineInner::Provisioned(_)) {
                    self.inner = PipelineInner::Provisioned(ProvisionedPipeline::new())
                }
            }
            _ => {}
        }
    }

    pub async fn process_inbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        pdu: PDU,
    ) -> Result<Option<State>, DeviceError> {
        self.inner.process_inbound(ctx, pdu).await
    }

    pub async fn process_outbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        message: &AccessMessage,
        publish: Option<(ModelKey, PublishRetransmitDetails)>,
        network_retransmit: NetworkRetransmitDetails,
    ) -> Result<(), DeviceError> {
        self.inner
            .process_outbound(ctx, message, publish, network_retransmit)
            .await
    }

    pub async fn retransmit<C: PipelineContext>(
        &mut self,
        ctx: &C,
        expiration: Expiration,
    ) -> Result<(), DeviceError> {
        self.inner.retransmit(ctx, expiration).await
    }
}
