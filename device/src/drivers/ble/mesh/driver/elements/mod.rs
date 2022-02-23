mod app_key;
mod beacon;
mod composition_data;
mod default_ttl;
mod model_app;
mod model_publication;
mod node_reset;

use crate::drivers::ble::mesh::address::UnicastAddress;
use crate::drivers::ble::mesh::composition::{Composition, ElementsHandler};
use crate::drivers::ble::mesh::config::Configuration;
use crate::drivers::ble::mesh::driver::node::OutboundPublishMessage;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::{
    ConfigurationMessage, ConfigurationServer,
};
use crate::drivers::ble::mesh::model::Message;
use crate::drivers::ble::mesh::model::Model;
use crate::drivers::ble::mesh::pdu::access::{AccessMessage, AccessPayload};
use core::cell::Ref;
use core::future::Future;
use core::marker::PhantomData;
use embassy::blocking_mutex::kind::Noop;
use embassy::channel::mpsc::Sender as ChannelSender;
use heapless::Vec;

pub struct AppElementsContext {
    pub(crate) sender: ChannelSender<'static, Noop, OutboundPublishMessage, 10>,
    pub(crate) address: UnicastAddress,
}

impl AppElementsContext {
    pub fn for_element_model<M: Model>(&self, element_number: u8) -> AppElementContext<M> {
        AppElementContext {
            sender: self.sender.clone(),
            address: self.address + element_number,
            _message: PhantomData,
        }
    }
}

#[derive(Clone)]
pub struct AppElementContext<M: Model> {
    sender: ChannelSender<'static, Noop, OutboundPublishMessage, 10>,
    address: UnicastAddress,
    _message: PhantomData<M>,
}

impl<M: Model> AppElementContext<M> {
    async fn transmit<'m>(&'m self, message: OutboundPublishMessage) -> Result<(), DeviceError> {
        self.sender
            .send(message)
            .await
            .map_err(|_| DeviceError::InsufficientBuffer)
    }

    pub async fn publish<'m>(&self, message: M::Message<'m>) -> Result<(), DeviceError> {
        let mut parameters = Vec::new();
        message.emit_parameters(&mut parameters)?;
        let publish = OutboundPublishMessage {
            element_address: self.address,
            model_identifier: M::IDENTIFIER,
            payload: AccessPayload {
                opcode: message.opcode(),
                parameters,
            },
        };
        self.transmit(publish).await
    }

    pub fn address(&self) -> UnicastAddress {
        self.address
    }
}

pub trait ElementContext {
    type TransmitFuture<'m>: Future<Output = Result<(), DeviceError>> + 'm
    where
        Self: 'm;

    fn transmit<'m>(&'m self, message: AccessMessage) -> Self::TransmitFuture<'m>;

    fn address(&self) -> Option<UnicastAddress>;
}

// todo: make primary significantly less special
pub trait PrimaryElementContext: ElementContext {
    type NodeResetFuture<'m>: Future<Output = ()>
    where
        Self: 'm;

    fn node_reset<'m>(&'m self) -> Self::NodeResetFuture<'m>;

    fn composition(&self) -> &Composition;

    fn configuration(&self) -> Ref<'_, Configuration>;

    type UpdateConfigurationFuture<'m, F>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm,
        F: 'm;

    fn update_configuration<F: FnOnce(&mut Configuration) -> Result<(), DeviceError>>(
        &self,
        update: F,
    ) -> Self::UpdateConfigurationFuture<'_, F>;
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
        if let Err(err) = self.zero.dispatch(ctx, message).await {
            error!("{}", err);
            Err(err)
        } else {
            Ok(())
        }
    }

    pub(crate) fn connect(&self, ctx: AppElementsContext) {
        self.app.connect(ctx);
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
                ConfigurationMessage::AppKey(message) => {
                    self::app_key::dispatch(ctx, access, message).await
                }
                ConfigurationMessage::ModelApp(message) => {
                    self::model_app::dispatch(ctx, access, message).await
                }
                ConfigurationMessage::ModelPublication(message) => {
                    self::model_publication::dispatch(ctx, access, message).await
                }
            }
        } else {
            // todo should probably be some UnhandledMessage error
            Ok(())
        }
    }
}
