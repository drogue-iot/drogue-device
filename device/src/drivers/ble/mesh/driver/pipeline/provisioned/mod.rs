use crate::drivers::ble::mesh::driver::node::deadline::Expiration;
use crate::drivers::ble::mesh::driver::node::State;
use crate::drivers::ble::mesh::driver::pipeline::mesh::{
    NetworkRetransmitDetails, PublishRetransmitDetails,
};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::access::AccessContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::lower::{Lower, LowerContext};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::authentication::{
    Authentication, AuthenticationContext,
};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::relay::{
    Relay, RelayContext,
};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::transmit::{
    ModelKey, Transmit,
};
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::NetworkContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::upper::{Upper, UpperContext};
use crate::drivers::ble::mesh::driver::pipeline::PipelineContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use crate::drivers::ble::mesh::pdu::network::ObfuscatedAndEncryptedNetworkPDU;
use futures::{join, pin_mut};

pub mod access;
pub mod lower;
pub mod network;
pub mod upper;

pub trait ProvisionedContext:
    AuthenticationContext + RelayContext + LowerContext + UpperContext + AccessContext + NetworkContext
{
}

pub(crate) struct ProvisionedPipeline {
    transmit: Transmit,
    authentication: Authentication,
    relay: Relay,
    lower: Lower,
    upper: Upper,
}

impl ProvisionedPipeline {
    pub(crate) fn new() -> Self {
        Self {
            transmit: Transmit::default(),
            authentication: Default::default(),
            relay: Default::default(),
            lower: Default::default(),
            upper: Default::default(),
        }
    }

    pub(crate) async fn process_inbound<C: PipelineContext>(
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
                    //ctx.transmit_mesh_pdu(&outbound).await.ok();
                    //ctx.enqueue_transmit(&outbound, ctx.relay_retransmit() ).await;
                    self.transmit
                        .process_outbound(ctx, outbound, &ctx.relay_retransmit());
                }
            }
        }
        Ok(None)
    }

    pub(crate) async fn process_outbound<C: PipelineContext>(
        &mut self,
        ctx: &C,
        message: &AccessMessage,
        publish: Option<(ModelKey, PublishRetransmitDetails)>,
        network_retransmit: NetworkRetransmitDetails,
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
            if let Some(message) = self.upper.process_outbound(ctx, message, publish)? {
                if let Some((seq_zero, message)) = self.lower.process_outbound(ctx, message).await?
                {
                    for message in message.iter() {
                        if let Some(message) = self.authentication.process_outbound(ctx, message)? {
                            self.transmit
                                .process_outbound(ctx, message, &network_retransmit)
                                .await?;
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

    pub async fn retransmit<C: PipelineContext>(
        &mut self,
        ctx: &C,
        expiration: Expiration,
    ) -> Result<(), DeviceError> {
        match expiration {
            Expiration::Network => self.transmit.retransmit(ctx).await,
            Expiration::Publish => self.upper.retransmit(ctx).await,
            Expiration::Ack => {
                if let Some(message) = self.lower.retransmit(ctx)? {
                    for message in message.iter() {
                        if let Some(message) = self.authentication.process_outbound(ctx, message)? {
                            self.transmit
                                .process_outbound(ctx, message, &ctx.network_retransmit())
                                .await?;
                        }
                    }
                }
                Ok(())
            }
        }
    }
}
