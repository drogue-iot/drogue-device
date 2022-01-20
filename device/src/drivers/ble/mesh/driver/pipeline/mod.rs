use crate::drivers::ble::mesh::driver::node::State;
use crate::drivers::ble::mesh::driver::pipeline::mesh::{Mesh, MeshData};
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::provisionable::{
    Provisionable, UnprovisionedContext,
};
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::provisioning_bearer::{
    BearerMessage, ProvisioningBearer,
};
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::access::Access;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::lower::Lower;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::authentication::Authentication;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::relay::Relay;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::ProvisionedContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::upper::Upper;
use crate::drivers::ble::mesh::generic_provisioning::Reason;
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
    access: Access,
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
            access: Default::default(),
        }
    }

    pub async fn process_inbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        data: &[u8],
    ) -> Result<Option<State>, DeviceError> {
        defmt::info!("pipeline {:x}", data);
        if let Some(result) = self.mesh.process_inbound(ctx, &data).await? {
            match result {
                MeshData::Provisioning(pdu) => {
                    defmt::info!("PDU {}", pdu);
                    if let Some(message) =
                        self.provisioning_bearer.process_inbound(ctx, pdu).await?
                    {
                        match message {
                            BearerMessage::ProvisioningPDU(provisioning_pdu) => {
                                defmt::info!("provisioning_pdu {}", provisioning_pdu);
                                if let Some(outbound) = self
                                    .provisionable
                                    .process_inbound(ctx, provisioning_pdu)
                                    .await?
                                {
                                    defmt::info!("<< outbound provisioning {}", outbound);
                                    for pdu in
                                        self.provisioning_bearer.process_outbound(outbound).await?
                                    {
                                        defmt::info!("<< outbound: {}", pdu);
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
                    defmt::info!("* {}", pdu);
                    if let Some(pdu) = self.authentication.process_inbound(ctx, pdu).await? {
                        defmt::info!("authenticated inbound -> {}", pdu);
                        // Relaying is independent from processing it locally
                        if let Some(outbound) = self.relay.process_inbound(ctx, &pdu).await? {

                        }

                        if let Some(pdu) = self.lower.process_inbound(ctx, pdu).await? {
                            defmt::info!("upper inbound --> {}", pdu);
                            if let Some(message) = self.upper.process_inbound(ctx, pdu).await? {
                                defmt::info!("inbound ----> {}", message);
                                if let Some(response) = self.access.process_inbound(ctx, message).await? {
                                    defmt::info!("outbound --> {}", response);
                                    // send it back outbound, finally.
                                    if let Some(response) = self.upper.process_outbound(ctx, response).await? {
                                        defmt::info!("outbound upper --> {}", response);
                                        if let Some(response) = self.lower.process_outbound(ctx, response).await? {
                                            defmt::info!("outbound lower --> {}", response);
                                            if let Some(response) = self.authentication.process_outbound(ctx, response).await? {
                                                defmt::info!("network --> {}", response);

                                                //for _ in 1..10 {
                                                    let result = ctx.transmit_mesh_pdu(&response).await;
                                                    defmt::info!("status {}", result);
                                                //}
                                            }
                                        }
                                    }
                                }
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

    pub async fn try_retransmit<C: PipelineContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        self.provisioning_bearer.try_retransmit(ctx).await
    }
}
