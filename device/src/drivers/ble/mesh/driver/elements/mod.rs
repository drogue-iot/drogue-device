pub mod configuration_server;

use crate::drivers::ble::mesh::driver::node::{Node, Receiver, Transmitter};
use crate::drivers::ble::mesh::element::{Element, ElementError, ElementSink};
use crate::drivers::ble::mesh::model::{Message, Model};
use crate::drivers::ble::mesh::model::foundation::configuration::{BeaconMessage, ConfigurationMessage, ConfigurationServer};
use crate::drivers::ble::mesh::pdu::access::{AccessPayload, Opcode};
use crate::drivers::ble::mesh::storage::Storage;
use rand_core::{CryptoRng, RngCore};
use crate::drivers::ble::mesh::driver::DeviceError;
use core::future::Future;
use core::marker::PhantomData;
use heapless::Vec;
use crate::drivers::ble::mesh::configuration_manager::{PrimaryElementModels, PrimaryElementStorage};

pub trait ElementContext {
    type TransmitFuture<'m>: Future<Output=Result<(), DeviceError> > + 'm
    where
    Self: 'm;

    fn transmit<'m, M: Message>(&'m self, message: &'m M) -> Self::TransmitFuture<'m> {
        let mut bytes = Vec::<u8, 384>::new();
        message.emit(&mut bytes).map_err(|_| DeviceError::InsufficientBuffer);
        self.transmit_bytes(bytes)
    }

    fn transmit_bytes<'m>(&'m self, message: Vec<u8, 384>) -> Self::TransmitFuture<'m>;
}

// todo: make primary significantly less special
pub trait PrimaryElementContext: ElementContext {
    fn retrieve(&self) -> PrimaryElementModels;

    type StoreFuture<'m> : Future<Output = Result<(), DeviceError>>
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

    pub(crate) async fn dispatch<C: PrimaryElementContext>(&self, ctx: &C, message: &AccessPayload) -> Result<(), DeviceError> {
        if let Ok(Some(message)) = self.primary.configuration_server.parse(message.opcode, &message.parameters) {
            match message {
                ConfigurationMessage::Beacon(beacon) => {
                    match beacon {
                        BeaconMessage::Get => {
                            let val = ctx.retrieve().configuration.secure_beacon;
                            ctx.transmit(&BeaconMessage::Status(val)).await?;
                        }
                        BeaconMessage::Set(val) => {
                            let mut update = ctx.retrieve();
                            update.configuration.secure_beacon = val;
                            ctx.store(update);
                            ctx.transmit(&BeaconMessage::Status(val)).await?;
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
