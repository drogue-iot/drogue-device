use crate::drivers::ble::mesh::driver::elements::ElementContext;
use crate::drivers::ble::mesh::pdu::access::{AccessMessage, Opcode};
use crate::drivers::ble::mesh::driver::DeviceError;
use core::future::Future;
use defmt::Format;
use crate::drivers::ble::mesh::model::{Model, ModelIdentifier};
use heapless::Vec;

pub trait ElementsHandler {
    fn composition(&self) -> &Composition;

    fn connect<C:ElementContext>(&self, ctx: &C);

    type DispatchFuture<'m>: Future<Output = Result<(), DeviceError>> + 'm
    where
    Self: 'm;

    fn dispatch<'m>(&'m self, element: u8, message: AccessMessage) -> Self::DispatchFuture<'m>;
}

#[derive(Copy, Clone, Format)]
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

#[derive(Clone, Format)]
pub struct Composition {
    cid: CompanyIdentifier,
    pid: ProductIdentifier,
    vid: VersionIdentifier,
    crpl: u16,
    features: Features,
    pub(crate) elements: Vec<ElementDescriptor, 10>,
}

impl Composition {
    pub fn new(cid: CompanyIdentifier, pid: ProductIdentifier, vid: VersionIdentifier, features: Features) -> Self {
        Self {
            cid,
            pid,
            vid,
            crpl: 0,
            features,
            elements: Default::default()
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
    loc: Location,
    models: Vec<ModelIdentifier, 10>,
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