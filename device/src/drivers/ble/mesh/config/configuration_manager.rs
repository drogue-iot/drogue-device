use crate::drivers::ble::mesh::address::Address;
use crate::drivers::ble::mesh::composition::{Composition, ElementDescriptor, Location};
use crate::drivers::ble::mesh::config::Configuration;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::CONFIGURATION_SERVER;
use crate::drivers::ble::mesh::storage::{Payload, Storage};
use atomic_polyfill::{AtomicBool, Ordering};
use core::cell::Ref;
use core::cell::RefCell;
use heapless::Vec;
use postcard::{from_bytes, to_slice};
use rand_core::{CryptoRng, RngCore};

pub(crate) const SEQUENCE_THRESHOLD: u32 = 100;

pub struct ConfigurationManager<S: Storage> {
    storage: RefCell<S>,
    config: RefCell<Configuration>,
    composition: Composition,
    runtime_seq: RefCell<u32>,
    force_reset: AtomicBool,
}

impl<S: Storage> ConfigurationManager<S> {
    pub fn new(storage: S, mut composition: Composition, force_reset: bool) -> Self {
        if composition.elements.is_empty() {
            let descriptor = ElementDescriptor::new(Location(0x0000));
            composition.add_element(descriptor).ok();
        }

        let mut models = Vec::new();
        models.push(CONFIGURATION_SERVER).ok();
        models
            .extend_from_slice(&composition.elements[0].models)
            .ok();
        composition.elements[0].models = models;

        let me = Self {
            storage: RefCell::new(storage),
            config: RefCell::new(Default::default()),
            composition,
            force_reset: AtomicBool::new(force_reset),
            runtime_seq: RefCell::new(0),
        };
        /*
        info!("CFG storage: {:?}", core::mem::size_of_val(&me.storage));
        info!("CFG config: {:?}", core::mem::size_of_val(&me.config));
        info!(
            "CFG composition: {:?}",
            core::mem::size_of_val(&me.composition)
        );
        info!(
            "CFG force_reset: {:?}",
            core::mem::size_of_val(&me.force_reset)
        );
        info!(
            "CFG runtime_seq: {:?}",
            core::mem::size_of_val(&me.runtime_seq)
        );*/
        me
    }

    pub(crate) async fn initialize<R: RngCore + CryptoRng>(
        &self,
        rng: &mut R,
    ) -> Result<(), DeviceError> {
        if self.force_reset.load(Ordering::SeqCst) {
            info!("Performing FORCE RESET");
            self.update_configuration(|config| {
                *config = Configuration::default();
                config.validate(rng);
                Ok(())
            })
            .await
        } else {
            let payload = self
                .storage
                .borrow_mut()
                .retrieve()
                .await
                .map_err(|_| DeviceError::StorageInitialization)?;
            match payload {
                None => {
                    info!("error loading configuration");
                    Err(DeviceError::StorageInitialization)
                }
                Some(payload) => {
                    let mut config: Configuration =
                        from_bytes(&payload.payload).map_err(|_| DeviceError::Serialization)?;
                    if config.validate(rng) {
                        // we initialized some things that we should stuff away.
                        self.runtime_seq.replace(config.seq);
                        self.update_configuration(move |stored| {
                            *stored = config.clone();
                            Ok(())
                        })
                        .await?;
                    } else {
                        self.runtime_seq.replace(config.seq);
                    }
                    Ok(())
                }
            }
        }
    }

    pub(crate) fn is_local_unicast(&self, addr: &Address) -> bool {
        match addr {
            Address::Unicast(addr) => {
                if let Some(network) = self.configuration().network() {
                    let primary: u16 = u16::from(*network.unicast_address());
                    let addr: u16 = u16::from(*addr);
                    addr >= primary && addr < primary + self.composition.elements.len() as u16
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    pub(crate) fn is_provisioned(&self) -> bool {
        self.configuration().network.is_some()
    }

    pub(crate) fn composition(&self) -> &Composition {
        &self.composition
    }

    pub(crate) async fn node_reset(&self) -> ! {
        // best effort
        self.update_configuration(|config| {
            *config = Configuration::default();
            Ok(())
        })
        .await
        .ok();
        // todo don't assume cortex-m some day
        #[cfg(cortex_m)]
        cortex_m::peripheral::SCB::sys_reset();

        #[cfg(not(cortex_m))]
        loop {}
    }

    #[cfg(feature = "defmt")]
    pub(crate) fn display_configuration(&self) {
        info!("================================================================");
        info!("Message Sequence: {}", *self.runtime_seq.borrow());
        self.config
            .borrow()
            .display_configuration(&self.composition);
        info!("================================================================");
    }

    pub(crate) fn configuration(&self) -> Ref<'_, Configuration> {
        self.config.borrow()
    }

    pub(crate) async fn update_configuration<
        F: FnOnce(&mut Configuration) -> Result<(), DeviceError>,
    >(
        &self,
        update: F,
    ) -> Result<(), DeviceError> {
        let mut config = self.config.borrow().clone();
        update(&mut config)?;
        *self.config.borrow_mut() = config;
        self.store().await
    }

    async fn store(&self) -> Result<(), DeviceError> {
        let mut payload = [0; 512];
        let config = self.config.borrow();
        to_slice(&*config, &mut payload)?;
        let payload = Payload { payload };
        self.storage
            .borrow_mut()
            .store(&payload)
            .await
            .map_err(|_| DeviceError::Storage)?;
        Ok(())
    }

    pub(crate) async fn next_sequence(&self) -> Result<u32, DeviceError> {
        let mut runtime_seq = self.runtime_seq.borrow_mut();
        let seq = *runtime_seq;
        *runtime_seq = *runtime_seq + 1;
        if *runtime_seq % SEQUENCE_THRESHOLD == 0 {
            self.update_configuration(|config| {
                config.seq = *runtime_seq;
                Ok(())
            })
            .await?;
        }
        Ok(seq)
    }

    pub(crate) fn reset(&self) {
        self.force_reset.store(true, Ordering::SeqCst);
    }
}
