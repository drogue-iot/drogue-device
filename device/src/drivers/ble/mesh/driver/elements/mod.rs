use crate::drivers::ble::mesh::address::UnicastAddress;
use crate::drivers::ble::mesh::configuration_manager::PrimaryElementModels;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::{
    BeaconMessage, ConfigurationMessage, ConfigurationServer,
};
use crate::drivers::ble::mesh::model::Model;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use core::future::Future;

pub trait ElementContext {
    type TransmitFuture<'m>: Future<Output = Result<(), DeviceError>> + 'm
    where
        Self: 'm;

    fn transmit<'m>(&'m self, message: AccessMessage) -> Self::TransmitFuture<'m>;

    fn address(&self) -> Option<UnicastAddress>;
}

// todo: make primary significantly less special
pub trait PrimaryElementContext: ElementContext {
    fn retrieve(&self) -> PrimaryElementModels;

    type StoreFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn store<'m>(&'m self, update: PrimaryElementModels) -> Self::StoreFuture<'m>;
}

pub struct Elements {
    primary: PrimaryElement,
}

impl Elements {
    pub fn new() -> Self {
        Self {
            primary: PrimaryElement::new(),
        }
    }

    pub(crate) async fn dispatch<C: PrimaryElementContext>(
        &self,
        ctx: &C,
        message: &AccessMessage,
    ) -> Result<(), DeviceError> {
        defmt::info!("primary elements");
        if let Ok(Some(payload)) = self
            .primary
            .configuration_server
            .parse(message.payload.opcode, &message.payload.parameters)
        {
            defmt::info!("HANDLE {}", payload);
            match payload {
                ConfigurationMessage::Beacon(beacon) => {
                    match beacon {
                        BeaconMessage::Get => {
                            defmt::info!("sending response to GET");
                            let val = ctx.retrieve().configuration.secure_beacon;
                            defmt::info!("sending response to GET --> {}", val);
                            //ctx.transmit(BeaconMessage::Status(val).into_outbound_access_message(message.src.into(), None)?).await?;
                            ctx.transmit(message.create_response(ctx, BeaconMessage::Status(val))?)
                                .await?;
                            defmt::info!("put on xmit queue");
                        }
                        BeaconMessage::Set(val) => {
                            defmt::info!("sending response to SET");
                            let mut update = ctx.retrieve();
                            update.configuration.secure_beacon = val;
                            ctx.store(update).await?;
                            //ctx.transmit(BeaconMessage::Status(val).into_outbound_access_message(message.src.into(), None)?).await?;
                            ctx.transmit(message.create_response(ctx, BeaconMessage::Status(val))?)
                                .await?;
                        }
                        _ => {
                            // not applicable to server role
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub struct PrimaryElement {
    configuration_server: ConfigurationServer,
}

impl PrimaryElement {
    fn new() -> Self {
        Self {
            configuration_server: ConfigurationServer,
        }
    }
}
