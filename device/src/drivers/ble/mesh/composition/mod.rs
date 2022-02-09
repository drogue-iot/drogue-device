use crate::drivers::ble::mesh::driver::elements::ElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::{Model, ModelIdentifier};
use crate::drivers::ble::mesh::pdu::access::{AccessMessage, Opcode};
use crate::drivers::ble::mesh::InsufficientBuffer;
use core::future::Future;
use defmt::Format;
use heapless::Vec;

pub trait ElementsHandler {
    fn composition(&self) -> &Composition;

    fn connect<C: ElementContext>(&self, ctx: &C);

    type DispatchFuture<'m>: Future<Output = Result<(), DeviceError>> + 'm
    where
        Self: 'm;

    fn dispatch<'m>(&'m self, element: u8, message: AccessMessage) -> Self::DispatchFuture<'m>;
}

#[derive(Eq, PartialEq, Copy, Clone, Format)]
pub struct CompanyIdentifier(pub u16);

#[derive(Copy, Clone, Format)]
pub struct ProductIdentifier(pub u16);

#[derive(Copy, Clone, Format)]
pub struct VersionIdentifier(pub u16);

#[derive(Copy, Clone, Format)]
pub struct Features {
    pub relay: bool,
    pub proxy: bool,
    pub friend: bool,
    pub low_power: bool,
}

impl Features {
    pub(crate) fn emit<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        // bits 15-8 RFU
        let mut val = 0;
        if self.relay {
            val = val | 0b0001;
        }
        if self.proxy {
            val = val | 0b0010;
        }
        if self.friend {
            val = val | 0b0100;
        }
        if self.low_power {
            val = val | 0b1000;
        }
        xmit.push(val).map_err(|_| InsufficientBuffer)?;
        xmit.push(0).map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}

#[derive(Clone, Format)]
pub struct Composition {
    pub(crate) cid: CompanyIdentifier,
    pub(crate) pid: ProductIdentifier,
    pub(crate) vid: VersionIdentifier,
    pub(crate) crpl: u16,
    pub(crate) features: Features,
    pub(crate) elements: Vec<ElementDescriptor, 10>,
}

impl Composition {
    pub fn new(
        cid: CompanyIdentifier,
        pid: ProductIdentifier,
        vid: VersionIdentifier,
        features: Features,
    ) -> Self {
        Self {
            cid,
            pid,
            vid,
            crpl: 0,
            features,
            elements: Default::default(),
        }
    }

    pub fn add_element(&mut self, element: ElementDescriptor) -> Result<(), ElementDescriptor> {
        self.elements.push(element)
    }
}

#[derive(Copy, Clone, Format)]
pub struct Location(pub u16);

#[derive(Clone, Format)]
pub struct ElementDescriptor {
    pub(crate) loc: Location,
    pub(crate) models: Vec<ModelIdentifier, 10>,
}

impl ElementDescriptor {
    pub fn new(loc: Location) -> Self {
        Self {
            loc,
            models: Default::default(),
        }
    }

    pub fn add_model(mut self, model: ModelIdentifier) -> Self {
        self.models.push(model);
        self
    }
}
