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
        if let Ok(Some(payload)) = self
            .primary
            .configuration_server
            .parse(message.payload.opcode, &message.payload.parameters)
        {
            match payload {
                ConfigurationMessage::Beacon(beacon) => {
                    match beacon {
                        BeaconMessage::Get => {
                            let val = ctx.retrieve().configuration.secure_beacon;
                            ctx.transmit(message.create_response(ctx, BeaconMessage::Status(val))?)
                                .await?;
                        }
                        BeaconMessage::Set(val) => {
                            let mut update = ctx.retrieve();
                            update.configuration.secure_beacon = val;
                            ctx.store(update).await?;
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
