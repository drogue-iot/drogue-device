use crate::drivers::ble::mesh::config::foundation_models::ConfigurationModel;
use crate::drivers::ble::mesh::driver::elements::AppElementsContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::ModelIdentifier;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use core::future::Future;
use heapless::Vec;
use serde::{Deserialize, Serialize};

pub trait ElementsHandler<'a> {
    fn composition(&self) -> &Composition;

    fn connect(&mut self, ctx: AppElementsContext<'a>);

    fn configure(&mut self, _: &ConfigurationModel) {}

    type DispatchFuture<'m>: Future<Output = Result<(), DeviceError>> + 'm
    where
        Self: 'm;

    fn dispatch<'m>(
        &'m mut self,
        element: u8,
        model_identifier: &'m ModelIdentifier,
        message: &'m AccessMessage,
    ) -> Self::DispatchFuture<'m>;
}

#[derive(Serialize, Deserialize, Eq, PartialEq, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CompanyIdentifier(pub u16);

impl CompanyIdentifier {
    pub fn parse(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() >= 2 {
            Ok(Self(u16::from_le_bytes([parameters[0], parameters[1]])))
        } else {
            Err(ParseError::InvalidLength)
        }
    }
}

#[derive(Serialize, Deserialize, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ProductIdentifier(pub u16);

#[derive(Serialize, Deserialize, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct VersionIdentifier(pub u16);

#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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

#[derive(Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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

#[derive(Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Location(pub u16);

#[derive(Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
        self.models.push(model).ok();
        self
    }
}
