use crate::drivers::ble::mesh::driver::node::State;
use crate::drivers::ble::mesh::driver::pipeline::mesh::{Mesh, MeshData};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::lower::Lower;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::authentication::Authentication;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::relay::Relay;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::upper::Upper;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::ProvisionedContext;
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
    ) -> Result<(), DeviceError> {
        match self {
            PipelineInner::Unconfigured => Err(DeviceError::NotProvisioned),
            PipelineInner::Unprovisioned(_) => Err(DeviceError::NotProvisioned),
            PipelineInner::Provisioned(inner) => inner.process_outbound(ctx, message).await,
        }
    }

    pub async fn try_retransmit<C: PipelineContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        match self {
            PipelineInner::Unconfigured => Err(DeviceError::PipelineNotConfigured),
            PipelineInner::Unprovisioned(inner) => inner.try_retransmit(ctx).await,
            PipelineInner::Provisioned(inner) => inner.try_retransmit(ctx).await,
        }
    }
}

struct UnprovisionedPipeline {
    provisioning_bearer: ProvisioningBearer,
    provisionable: Provisionable,
}

impl UnprovisionedPipeline {
    fn new(capabilities: Capabilities) -> Self {
        Self {
            provisioning_bearer: Default::default(),
            provisionable: Provisionable::new(capabilities),
        }
    }

    async fn process_inbound<C: PipelineContext>(
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

    async fn try_retransmit<C: PipelineContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        self.provisioning_bearer.try_retransmit(ctx).await
    }
}

struct ProvisionedPipeline {
    authentication: Authentication,
    relay: Relay,
    lower: Lower,
    upper: Upper,
}

impl ProvisionedPipeline {
    fn new() -> Self {
        Self {
            authentication: Default::default(),
            relay: Default::default(),
            lower: Default::default(),
            upper: Default::default(),
        }
    }

    async fn process_inbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        pdu: &mut ObfuscatedAndEncryptedNetworkPDU,
    ) -> Result<Option<State>, DeviceError> {
        if let Some(inboud_pdu) = self.authentication.process_inbound(ctx, pdu)? {
            let (ack, pdu) = self.lower.process_inbound(ctx, &inboud_pdu).await?;

            if let Some(pdu) = pdu {
                if let Some(message) = self.upper.process_inbound(ctx, pdu)? {
                    ctx.dispatch_access(&message).await?;
                }
            }

            if let Some(ack) = ack {
                if let Some(ack) = self.authentication.process_outbound(ctx, &ack)? {
                    // don't fail if we fail to transmit the ack.
                    ctx.transmit_mesh_pdu(&ack).await.ok();
                }
            }

            // Relaying is independent from processing it locally
            if let Some(outbound) = self.relay.process_inbound(ctx, &inboud_pdu)? {
                // don't fail if we fail to encrypt a relay.
                if let Ok(Some(outbound)) = self.authentication.process_outbound(ctx, &outbound) {
                    // don't fail if we fail to retransmit.
                    ctx.transmit_mesh_pdu(&outbound).await.ok();
                }
            }
        }
        Ok(None)
    }

    async fn process_outbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        message: &AccessMessage,
    ) -> Result<(), DeviceError> {
        trace!("outbound <<<< {}", message);

        // local loopback.
        let loopback_fut = async move {
            info!("l>");
            if ctx.is_locally_relevant(&message.dst) {
                ctx.dispatch_access(&message).await?;
            }
            info!("l<");
            Result::<(), DeviceError>::Ok(())
        };

        let transmit_fut = async move {
            if let Some(message) = self.upper.process_outbound(ctx, message)? {
                if let Some(message) = self.lower.process_outbound(ctx, message).await? {
                    for message in message.iter() {
                        if let Some(message) = self.authentication.process_outbound(ctx, message)? {
                            ctx.transmit_mesh_pdu(&message).await?;
                        }
                    }
                }
            }
            Result::<(), DeviceError>::Ok(())
        };

        pin_mut!(loopback_fut);
        pin_mut!(transmit_fut);

        let result = join!(loopback_fut, transmit_fut);

        match result {
            (Ok(()), Ok(())) => Ok(()),
            (_, Err(e)) => Err(e),
            (Err(e), _) => Err(e),
        }
    }

    pub async fn try_retransmit<C: PipelineContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        if let Some(message) = self.lower.process_retransmits()? {
            for message in message.iter() {
                if let Some(message) = self.authentication.process_outbound(ctx, message)? {
                    ctx.transmit_mesh_pdu(&message).await?;
                }
            }
        }
        Ok(())
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
    ) -> Result<(), DeviceError> {
        self.inner.process_outbound(ctx, message).await
    }

    pub async fn try_retransmit<C: PipelineContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        self.inner.try_retransmit(ctx).await
    }
}
