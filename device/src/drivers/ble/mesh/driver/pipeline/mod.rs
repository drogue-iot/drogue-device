use crate::drivers::ble::mesh::driver::node::State;
use crate::drivers::ble::mesh::driver::pipeline::mesh::{Mesh, MeshData};
use crate::drivers::ble::mesh::driver::pipeline::provisionable::{
    Provisionable, ProvisionableContext,
};
use crate::drivers::ble::mesh::driver::pipeline::provisioning_bearer::{
    BearerMessage, ProvisioningBearer,
};
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::generic_provisioning::Reason;
use crate::drivers::ble::mesh::provisioning::Capabilities;

pub mod mesh;
pub mod provisionable;
pub mod provisioning_bearer;
pub mod segmentation;

pub trait PipelineContext: ProvisionableContext {}

pub struct Pipeline {
    mesh: Mesh,
    provisioning_bearer: ProvisioningBearer,
    provisionable: Provisionable,
}

impl Pipeline {
    pub fn new(capabilities: Capabilities) -> Self {
        Self {
            mesh: Default::default(),
            provisioning_bearer: Default::default(),
            provisionable: Provisionable::new(capabilities),
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
            }
        } else {
            Ok(None)
        }
    }

    pub async fn try_retransmit<C: PipelineContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        self.provisioning_bearer.try_retransmit(ctx).await
    }
}
