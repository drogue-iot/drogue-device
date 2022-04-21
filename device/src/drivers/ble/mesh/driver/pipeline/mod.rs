use crate::drivers::ble::mesh::driver::node::State;
use crate::drivers::ble::mesh::driver::pipeline::mesh::{Mesh, MeshData, NetworkRetransmitDetails, PublishRetransmitDetails};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::lower::Lower;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::authentication::Authentication;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::relay::Relay;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::upper::Upper;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::{ProvisionedContext, ProvisionedPipeline};
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::provisionable::{
    Provisionable, UnprovisionedContext,
};
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::provisioning_bearer::{
    BearerMessage, ProvisioningBearer,
};
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::generic_provisioning::Reason;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use crate::drivers::ble::mesh::pdu::bearer::advertising::AdvertisingPDU;
use crate::drivers::ble::mesh::pdu::network::ObfuscatedAndEncryptedNetworkPDU;
use crate::drivers::ble::mesh::provisioning::Capabilities;
use futures::{join, pin_mut};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::transmit::ModelKey;
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::UnprovisionedPipeline;


pub mod mesh;
pub mod provisioned;
pub mod unprovisioned;

pub trait PipelineContext: UnprovisionedContext + ProvisionedContext {}

pub struct Pipeline {
    mesh: Mesh,
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
        message: &mut MeshData,
    ) -> Result<Option<State>, DeviceError> {
        match self {
            PipelineInner::Unconfigured => Ok(None),
            PipelineInner::Unprovisioned(inner) => {
                if let MeshData::Provisioning(pdu) = message {
                    inner.process_inbound(ctx, pdu).await
                } else {
                    Ok(None)
                }
            }
            PipelineInner::Provisioned(inner) => {
                if let MeshData::Network(pdu) = message {
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
            PipelineInner::Provisioned(inner) => inner.process_outbound(ctx, message, publish, network_retransmit).await,
        }
    }

    // todo: percolate retransmits up entirely to a retransmit queue instead of holding them within the pipeline.
    pub async fn try_retransmit<C: PipelineContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        match self {
            PipelineInner::Unconfigured => Err(DeviceError::PipelineNotConfigured),
            PipelineInner::Unprovisioned(inner) => inner.try_retransmit(ctx).await,
            PipelineInner::Provisioned(inner) => inner.try_retransmit(ctx).await,
        }
    }
}


impl Pipeline {
    pub fn new(capabilities: Capabilities) -> Self {
        Self {
            mesh: Default::default(),
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
        data: &[u8],
    ) -> Result<Option<State>, DeviceError> {
        if let Some(mut result) = self.mesh.process_inbound(ctx, &data)? {
            self.inner.process_inbound(ctx, &mut result).await
        } else {
            Ok(None)
        }
    }

    pub async fn process_outbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        message: &AccessMessage,
        publish: Option<(ModelKey, PublishRetransmitDetails)>,
        network_retransmit: NetworkRetransmitDetails,
    ) -> Result<(), DeviceError> {
        self.inner.process_outbound(ctx, message, publish, network_retransmit).await
    }

    pub async fn try_retransmit<C: PipelineContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        self.inner.try_retransmit(ctx).await
    }
}
