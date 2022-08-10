mod app_key;
mod beacon;
mod composition_data;
mod default_ttl;
mod model_app;
mod model_publication;
mod model_subscription;
mod node_reset;
#[cfg(feature = "ble-mesh-relay")]
mod relay;

use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
use crate::drivers::ble::mesh::composition::{Composition, ElementsHandler};
use crate::drivers::ble::mesh::config::Configuration;
use crate::drivers::ble::mesh::driver::node::outbound::OutboundPublishMessage;
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
use embassy_util::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_util::channel::mpmc::Sender as ChannelSender;
use heapless::Vec;

#[derive(Clone)]
pub struct AppElementsContext<'a> {
    pub(crate) sender: ChannelSender<'a, ThreadModeRawMutex, OutboundPublishMessage, 1>,
    pub(crate) access_sender: ChannelSender<'a, ThreadModeRawMutex, AccessMessage, 1>,
    pub(crate) address: UnicastAddress,
}

impl<'a> AppElementsContext<'a> {
    pub fn for_element_model<M: Model>(&self, element_number: u8) -> AppElementContext<'a, M> {
        AppElementContext {
            sender: self.sender.clone(),
            address: self.address + element_number,
            _message: PhantomData,
        }
    }

    pub fn address(&self) -> UnicastAddress {
        self.address
    }

    pub async fn respond<M: Message>(
        &self,
        access: &AccessMessage,
        response: M,
    ) -> Result<(), DeviceError> {
        let m = access.create_response(self.address, response)?;
        self.access_sender.send(m).await;
        Ok(())
    }
}

#[derive(Clone)]
pub struct AppElementContext<'a, M: Model> {
    sender: ChannelSender<'a, ThreadModeRawMutex, OutboundPublishMessage, 1>,
    address: UnicastAddress,
    _message: PhantomData<M>,
}

impl<'a, M: Model> AppElementContext<'a, M> {
    async fn transmit<'m>(&'m self, message: OutboundPublishMessage) -> Result<(), DeviceError> {
        self.sender.send(message).await;
        Ok(())
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

pub struct Elements<'a, E: ElementsHandler<'a>> {
    zero: ElementZero,
    pub(crate) elements: E,
    _a: PhantomData<&'a E>,
}

impl<'a, E: ElementsHandler<'a>> Elements<'a, E> {
    pub fn new(app_elements: E) -> Self {
        Self {
            zero: ElementZero::new(),
            elements: app_elements,
            _a: PhantomData,
        }
    }

    pub(crate) async fn dispatch<C: PrimaryElementContext>(
        &mut self,
        ctx: &C,
        message: &AccessMessage,
    ) -> Result<(), DeviceError> {
        info!("d>");
        let composition = self.elements.composition().clone();
        let unicast_element_index = match &message.dst {
            Address::Unicast(addr) => {
                let primary_addr = ctx.address().ok_or(DeviceError::NotProvisioned)?;
                if *addr >= primary_addr {
                    let element_index = *addr - primary_addr;
                    if element_index < composition.elements.len() as u8 {
                        Some(element_index)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(element_index) = unicast_element_index {
            if element_index == 0 {
                if self.zero.dispatch(ctx, message).await? {
                    info!("d<");
                    self.elements.configure(
                        &ctx.configuration()
                            .foundation_models()
                            .configuration_model(),
                    );
                    return Ok(());
                }
            }
            let element = &composition.elements[element_index as usize];
            for model in &element.models {
                self.elements
                    .dispatch(element_index as u8, model, message)
                    .await?;
            }
            info!("d<");
            return Ok(());
        }

        if let Some(network) = ctx.configuration().network() {
            let primary_addr = ctx.address().ok_or(DeviceError::NotProvisioned)?;
            // non-unicast, give everyone a chance to respond
            for (element_index, element) in composition.elements.iter().enumerate() {
                // non-unicast, check every element.
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
                        self.elements
                            .dispatch(element_index as u8, model, message)
                            .await?;
                    }
                }
            }
        }

        info!("d<");
        Ok(())
    }

    pub(crate) fn connect(&mut self, ctx: AppElementsContext<'a>) {
        self.elements.connect(ctx);
    }
}

pub struct ElementZero {}

impl ElementZero {
    fn new() -> Self {
        Self {}
    }
}

impl ElementZero {
    pub(crate) async fn dispatch<C: PrimaryElementContext>(
        &self,
        ctx: &C,
        access: &AccessMessage,
    ) -> Result<bool, DeviceError> {
        if let Ok(Some(payload)) =
            ConfigurationServer::parse(access.payload.opcode, &access.payload.parameters)
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
                #[cfg(feature = "ble-mesh-relay")]
                ConfigurationMessage::Relay(message) => {
                    self::relay::dispatch(ctx, access, message).await?;
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
