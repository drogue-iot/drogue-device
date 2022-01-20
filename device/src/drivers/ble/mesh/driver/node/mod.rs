use crate::drivers::ble::mesh::configuration_manager::ConfigurationManager;
use crate::drivers::ble::mesh::driver::pipeline::Pipeline;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::Capabilities;
use crate::drivers::ble::mesh::storage::Storage;
use crate::drivers::ble::mesh::vault::{StorageVault, Vault};
use crate::drivers::ble::mesh::MESH_BEACON;
use core::cell::RefCell;
use core::future::Future;
use embassy::time::{Duration, Ticker};
use futures::future::{select, Either};
use futures::{pin_mut, StreamExt};
use heapless::Vec;
use rand_core::{CryptoRng, RngCore};

mod context;

pub trait Transmitter {
    type TransmitFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;
    fn transmit_bytes<'m>(&'m self, bytes: &'m [u8]) -> Self::TransmitFuture<'m>;
}

pub trait Receiver {
    type ReceiveFuture<'m>: Future<Output = Result<Vec<u8, 384>, DeviceError>>
    where
        Self: 'm;
    fn receive_bytes<'m>(&'m self) -> Self::ReceiveFuture<'m>;
}

pub enum State {
    Unprovisioned,
    Provisioning,
    Provisioned,
}

pub struct Node<TX, RX, S, R>
where
    TX: Transmitter,
    RX: Receiver,
    S: Storage,
    R: RngCore + CryptoRng,
{
    state: State,
    //
    transmitter: TX,
    receiver: RX,
    configuration_manager: ConfigurationManager<S>,
    rng: RefCell<R>,
    pipeline: RefCell<Pipeline>,
}

impl<TX, RX, S, R> Node<TX, RX, S, R>
where
    TX: Transmitter,
    RX: Receiver,
    S: Storage,
    R: RngCore + CryptoRng,
{
    pub fn new(
        capabilities: Capabilities,
        transmitter: TX,
        receiver: RX,
        configuration_manager: ConfigurationManager<S>,
        rng: R,
    ) -> Self {
        Self {
            state: State::Unprovisioned,
            transmitter,
            receiver: receiver,
            configuration_manager,
            rng: RefCell::new(rng),
            pipeline: RefCell::new(Pipeline::new(capabilities)),
        }
    }

    pub(crate) fn vault(&self) -> StorageVault<ConfigurationManager<S>> {
        StorageVault::new(&self.configuration_manager)
    }

    async fn loop_unprovisioned(&mut self) -> Result<Option<State>, DeviceError> {
        self.transmit_unprovisioned_beacon().await?;

        let receive_fut = self.receiver.receive_bytes();

        let mut ticker = Ticker::every(Duration::from_secs(3));
        let ticker_fut = ticker.next();

        pin_mut!(receive_fut);
        pin_mut!(ticker_fut);

        let result = select(receive_fut, ticker_fut).await;

        match result {
            Either::Left((Ok(msg), _)) => {
                self.pipeline
                    .borrow_mut()
                    .process_inbound(self, &*msg)
                    .await
            }
            Either::Right((_, _)) => {
                self.transmit_unprovisioned_beacon().await?;
                Ok(None)
            }
            _ => {
                // TODO handle this
                Ok(None)
            }
        }
    }

    async fn transmit_unprovisioned_beacon(&self) -> Result<(), DeviceError> {
        let mut adv_data: Vec<u8, 31> = Vec::new();
        adv_data.extend_from_slice(&[20, MESH_BEACON, 0x00]).ok();
        adv_data.extend_from_slice(&self.vault().uuid().0).ok();
        adv_data.extend_from_slice(&[0xa0, 0x40]).ok();

        self.transmitter.transmit_bytes(&*adv_data).await
    }

    async fn loop_provisioning(&mut self) -> Result<Option<State>, DeviceError> {
        let receive_fut = self.receiver.receive_bytes();
        let mut ticker = Ticker::every(Duration::from_secs(1));
        let ticker_fut = ticker.next();

        pin_mut!(receive_fut);
        pin_mut!(ticker_fut);

        let result = select(receive_fut, ticker_fut).await;

        match result {
            Either::Left((Ok(msg), _)) => {
                self.pipeline
                    .borrow_mut()
                    .process_inbound(self, &*msg)
                    .await
            }
            Either::Right((_, _)) => {
                self.pipeline.borrow_mut().try_retransmit(self).await?;
                Ok(None)
            }
            _ => {
                // TODO handle this
                Ok(None)
            }
        }
    }

    async fn loop_provisioned(&mut self) -> Result<Option<State>, DeviceError> {
        Ok(None)
    }

    pub async fn run(&mut self) -> Result<(), ()> {
        // stop right now if we can't initialize our configuration manager.
        self.configuration_manager
            .initialize(&mut *self.rng.borrow_mut())
            .await
            .map_err(|_| ())?;

        loop {
            if let Ok(Some(next_state)) = match self.state {
                State::Unprovisioned => self.loop_unprovisioned().await,
                State::Provisioning => self.loop_provisioning().await,
                State::Provisioned => self.loop_provisioned().await,
            } {
                self.state = next_state;
            }
        }
    }
}
