use crate::drivers::ble::mesh::address::UnicastAddress;
use crate::drivers::ble::mesh::model::foundation::configuration::AppKeyIndex;
use crate::drivers::ble::mesh::model::{ModelIdentifier, Status};
use heapless::Vec;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Default, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Bindings {
    bindings: Vec<Binding, 10>,
}

impl Bindings {
    pub(crate) fn display_bindings(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) {
        let mut matching: Vec<&Binding, 20> = Vec::new();
        for e in self.bindings.iter().filter(|e| {
            e.element_address == *element_address && e.model_identifier == *model_identifier
        }) {
            matching.push(e).ok();
        }

        if !matching.is_empty() {
            info!("      App Keys:");
            for binding in matching.iter() {
                info!("        [{:?}]", binding.app_key_index);
            }
        }
    }

    fn find(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) -> Option<&Binding> {
        self.bindings.iter().find(|e| {
            e.element_address == *element_address && e.model_identifier == *model_identifier
        })
    }

    fn find_mut(
        &mut self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) -> Option<&mut Binding> {
        self.bindings.iter_mut().find(|e| {
            e.element_address == *element_address && e.model_identifier == *model_identifier
        })
    }

    pub fn bind(
        &mut self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
        app_key_index: &AppKeyIndex,
    ) -> Result<(), Status> {
        if let Some(previous) = self.find_mut(element_address, model_identifier) {
            previous.app_key_index = *app_key_index
        } else {
            let binding = Binding {
                model_identifier: *model_identifier,
                element_address: *element_address,
                app_key_index: *app_key_index,
            };
            self.bindings
                .push(binding)
                .map_err(|_| Status::InsufficientResources)?
        }
        Ok(())
    }

    pub fn unbind(
        &mut self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) -> Result<(), Status> {
        let mut removed = false;
        let mut bindings = Vec::new();
        for binding in self.bindings.iter() {
            if !binding.matches(element_address, model_identifier) {
                bindings
                    .push(*binding)
                    .map_err(|_| Status::InsufficientResources)?;
                removed = true
            }
        }

        if removed {
            self.bindings = bindings;
            Ok(())
        } else {
            Err(Status::InvalidBinding)
        }
    }
}

#[derive(Serialize, Deserialize, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Binding {
    model_identifier: ModelIdentifier,
    element_address: UnicastAddress,
    app_key_index: AppKeyIndex,
}

impl Binding {
    fn matches(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) -> bool {
        self.element_address == *element_address && self.model_identifier == *model_identifier
    }

    pub fn model_identifier(&self) -> ModelIdentifier {
        self.model_identifier
    }

    pub fn element_address(&self) -> UnicastAddress {
        self.element_address
    }

    pub fn app_key_index(&self) -> &AppKeyIndex {
        &self.app_key_index
    }

    pub fn app_key_index_mut(&mut self) -> &mut AppKeyIndex {
        &mut self.app_key_index
    }
}
