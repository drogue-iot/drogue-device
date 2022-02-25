use crate::drivers::ble::mesh::address::UnicastAddress;
use crate::drivers::ble::mesh::model::foundation::configuration::model_subscription::SubscriptionAddress;
use crate::drivers::ble::mesh::model::{ModelIdentifier, Status};
use heapless::Vec;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Subscriptions {
    subscriptions: Vec<Subscription, 20>,
}

impl Default for Subscriptions {
    fn default() -> Self {
        Self {
            subscriptions: Default::default(),
        }
    }
}

impl Subscriptions {
    #[cfg(feature = "defmt")]
    pub(crate) fn display_subscriptions(
        &self,
        element_address: &UnicastAddress,
        model_identifier: &ModelIdentifier,
    ) {
        let mut matching: Vec<&Subscription, 20> = Vec::new();
        for e in self.subscriptions.iter().filter(|e| {
            e.element_address == *element_address && e.model_identifier == *model_identifier
        }) {
            matching.push(e).ok();
        }

        if !matching.is_empty() {
            info!("      Subscriptions:");
            for subscription in matching.iter() {
                info!("        {}", subscription.subscription_address,);
            }
        }
    }

    pub(crate) fn add(
        &mut self,
        element_address: UnicastAddress,
        subscription_address: SubscriptionAddress,
        model_identifier: ModelIdentifier,
    ) -> Result<(), Status> {
        if let Some(existing) = self.subscriptions.iter().find(|e| {
            e.element_address == element_address
                && e.subscription_address == subscription_address
                && e.model_identifier == model_identifier
        }) {
            // no harm, no foul
            Ok(())
        } else {
            self.subscriptions
                .push(Subscription {
                    element_address,
                    subscription_address,
                    model_identifier,
                })
                .map_err(|_| Status::InsufficientResources)?;
            Ok(())
        }
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Subscription {
    element_address: UnicastAddress,
    subscription_address: SubscriptionAddress,
    model_identifier: ModelIdentifier,
}
