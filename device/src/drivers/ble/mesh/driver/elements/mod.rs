mod beacon;
mod default_ttl;
mod node_reset;
mod composition_data;

use crate::drivers::ble::mesh::address::UnicastAddress;
use crate::drivers::ble::mesh::composition::{Composition, ElementsHandler};
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

    type NodeResetFuture<'m>: Future<Output = ()>
    where
        Self: 'm;

    fn node_reset<'m>(&'m self) -> Self::NodeResetFuture<'m>;

    fn composition(&self) -> &Composition;
}

pub struct Elements<E: ElementsHandler> {
    zero: ElementZero,
    pub(crate) app: E,
}

impl<E: ElementsHandler> Elements<E> {
    pub fn new(app_elements: E) -> Self {
        Self {
            zero: ElementZero::new(),
            app: app_elements,
        }
    }

    pub(crate) async fn dispatch<C: PrimaryElementContext>(
        &self,
        ctx: &C,
        message: &AccessMessage,
    ) -> Result<(), DeviceError> {
        // todo dispatch correctly based on dst address element
        self.zero.dispatch(ctx, message).await
    }
}

pub struct ElementZero {
    configuration_server: ConfigurationServer,
}

impl ElementZero {
    fn new() -> Self {
        Self {
            configuration_server: ConfigurationServer,
        }
    }
}

impl ElementZero {
    pub(crate) async fn dispatch<C: PrimaryElementContext>(
        &self,
        ctx: &C,
        access: &AccessMessage,
    ) -> Result<(), DeviceError> {
        if let Ok(Some(payload)) = self
            .configuration_server
            .parse(access.payload.opcode, &access.payload.parameters)
        {
            match &payload {
                ConfigurationMessage::Beacon(message) => {
                    self::beacon::dispatch(ctx, access, message).await
                }
                ConfigurationMessage::DefaultTTL(message) => {
                    self::default_ttl::dispatch(ctx, access, message).await
                }
                ConfigurationMessage::NodeReset(message) => {
                    self::node_reset::dispatch(ctx, access, message).await
                }
                ConfigurationMessage::CompositionData(message) => {
                    self::composition_data::dispatch(ctx, access, message).await
                }
            }
        } else {
            // todo should probably be some UnhandledMessage error
            Ok(())
        }
    }
}
