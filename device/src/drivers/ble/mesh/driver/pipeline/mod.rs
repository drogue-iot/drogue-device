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
use crate::drivers::ble::mesh::provisioning::Capabilities;

pub mod mesh;
pub mod provisioned;
pub mod unprovisioned;

pub trait PipelineContext: UnprovisionedContext + ProvisionedContext {}

pub struct Pipeline {
    mesh: Mesh,
    // unprovisioned pipeline
    provisioning_bearer: ProvisioningBearer,
    provisionable: Provisionable,
    // provisioned pipeline
    authentication: Authentication,
    relay: Relay,
    lower: Lower,
    upper: Upper,
}

impl Pipeline {
    pub fn new(capabilities: Capabilities) -> Self {
        Self {
            mesh: Default::default(),
            provisioning_bearer: Default::default(),
            provisionable: Provisionable::new(capabilities),
            //
            authentication: Default::default(),
            relay: Default::default(),
            lower: Default::default(),
            upper: Default::default(),
        }
    }

    pub async fn process_inbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        data: &[u8],
    ) -> Result<Option<State>, DeviceError> {
        if let Some(result) = self.mesh.process_inbound(ctx, &data).await? {
            match result {
                MeshData::Provisioning(pdu) => {
                    if let Some(message) =
                        self.provisioning_bearer.process_inbound(ctx, pdu).await?
                    {
                        match message {
                            BearerMessage::ProvisioningPDU(provisioning_pdu) => {
                                if let Some(outbound) = self
                                    .provisionable
                                    .process_inbound(ctx, provisioning_pdu)
                                    .await?
                                {
                                    for pdu in
                                        self.provisioning_bearer.process_outbound(outbound).await?
                                    {
                                        self.mesh.process_outbound(ctx, pdu).await?;
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
                MeshData::Network(pdu) => {
                    if let Some(pdu) = self.authentication.process_inbound(ctx, pdu).await? {
                        // Relaying is independent from processing it locally
                        if let Some(outbound) = self.relay.process_inbound(ctx, &pdu).await? {
                            // don't fail if we fail to encrypt a relay.
                            if let Ok(Some(outbound)) =
                                self.authentication.process_outbound(ctx, &outbound).await
                            {
                                // don't fail if we fail to retransmit.
                                ctx.transmit_mesh_pdu(&outbound).await.ok();
                            }
                        }

                        let (ack, pdu) = self.lower.process_inbound(ctx, pdu).await?;
                        if let Some(ack) = ack {
                            if let Some(ack) =
                                self.authentication.process_outbound(ctx, &ack).await?
                            {
                                // don't fail if we fail to transmit the ack.
                                ctx.transmit_mesh_pdu(&ack).await.ok();
                            }
                        }

                        if let Some(pdu) = pdu {
                            if let Some(message) = self.upper.process_inbound(ctx, pdu).await? {
                                ctx.dispatch_access(&message).await?;
                            }
                        }
                    }
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }

    pub async fn process_outbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        message: &AccessMessage,
    ) -> Result<(), DeviceError> {
        trace!("outbound <<<< {}", message);

        // local loopback.
        if ctx.is_locally_relevant(&message.dst) {
            ctx.dispatch_access(&message).await?;
        }

        if let Some(message) = self.upper.process_outbound(ctx, message).await? {
            if let Some(message) = self.lower.process_outbound(ctx, message).await? {
                for message in message.iter() {
                    if let Some(message) =
                        self.authentication.process_outbound(ctx, message).await?
                    {
                        ctx.transmit_mesh_pdu(&message).await?;
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn try_retransmit<C: PipelineContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        self.provisioning_bearer.try_retransmit(ctx).await
    }
}
