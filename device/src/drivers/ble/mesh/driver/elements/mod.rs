mod app_key;
mod beacon;
mod composition_data;
mod default_ttl;
mod model_app;
mod model_publication;
mod model_subscription;
mod node_reset;

use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
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
use core::convert::TryInto;
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

    fn is_local(&self, addr: &UnicastAddress) -> bool;
}

pub struct Elements<E: ElementsHandler> {
    zero: ElementZero,
    pub(crate) elements: E,
}

impl<E: ElementsHandler> Elements<E> {
    pub fn new(app_elements: E) -> Self {
        Self {
            zero: ElementZero::new(),
            elements: app_elements,
        }
    }

    pub(crate) async fn dispatch<C: PrimaryElementContext>(
        &self,
        ctx: &C,
        message: &AccessMessage,
    ) -> Result<(), DeviceError> {
        let unicast_element_index = match &message.dst {
            Address::Unicast(addr) => {
                let primary_addr = ctx.address().ok_or(DeviceError::NotProvisioned)?;
                let element_index = *addr - primary_addr;
                if element_index < self.elements.composition().elements.len() as u8 {
                    Some(element_index)
                } else {
                    None
                }
            }
            _ => None,
        };

        for (element_index, element) in self.elements.composition().elements.iter().enumerate() {
            if let Some(unicast_element_index) = unicast_element_index {
                // it's a unicast-directed message... is it a given element's?
                if unicast_element_index == element_index as u8 {
                    if element_index == 0 {
                        // try to dispatch to foundation models in element#0
                        if self.zero.dispatch(ctx, message).await? {
                            // handled, stop, don't pass to app models on element#0
                            return Ok(());
                        }
                    } else {
                        // the app gets one chance to process.
                        self.elements.dispatch(element_index as u8, message).await?;
                        return Ok(());
                    }
                }
            } else {
                let primary_addr = ctx.address().ok_or(DeviceError::NotProvisioned)?;
                // non-unicast, check every element.
                if let Some(network) = ctx.configuration().network() {
                    let element_address = primary_addr + element_index as u8;
                    for model in &element.models {
                        if network.subscriptions().has_subscription(
                            &element_address,
                            &message
                                .dst
                                .try_into()
                                .map_err(|_| DeviceError::InvalidDstAddress)?,
                            model,
                        ) {
                            self.elements.dispatch(element_index as u8, message).await?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    pub(crate) fn connect(&self, ctx: AppElementsContext) {
        self.elements.connect(ctx);
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
    ) -> Result<bool, DeviceError> {
        if let Ok(Some(payload)) = self
            .configuration_server
            .parse(access.payload.opcode, &access.payload.parameters)
        {
            match &payload {
                ConfigurationMessage::Beacon(message) => {
                    self::beacon::dispatch(ctx, access, message).await?;
                }
                ConfigurationMessage::DefaultTTL(message) => {
                    self::default_ttl::dispatch(ctx, access, message).await?;
                }
                ConfigurationMessage::NodeReset(message) => {
                    self::node_reset::dispatch(ctx, access, message).await?;
                }
                ConfigurationMessage::CompositionData(message) => {
                    self::composition_data::dispatch(ctx, access, message).await?;
                }
                ConfigurationMessage::AppKey(message) => {
                    self::app_key::dispatch(ctx, access, message).await?;
                }
                ConfigurationMessage::ModelApp(message) => {
                    self::model_app::dispatch(ctx, access, message).await?;
                }
                ConfigurationMessage::ModelPublication(message) => {
                    self::model_publication::dispatch(ctx, access, message).await?;
                }
                ConfigurationMessage::ModelSubscription(message) => {
                    self::model_subscription::dispatch(ctx, access, message).await?;
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
