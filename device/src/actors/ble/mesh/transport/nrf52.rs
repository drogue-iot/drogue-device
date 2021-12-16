use crate::drivers::ble::mesh::transport::nrf52::Nrf52BleMeshTransport;
use crate::{Actor, Address, Inbox};
use core::future::Future;
use nrf_softdevice::Softdevice;

impl Nrf52BleMeshTransport {
    pub fn actor(&self) -> Nrf52BleMeshTransportActor {
        Nrf52BleMeshTransportActor(self.sd)
    }
}

pub struct Nrf52BleMeshTransportActor(&'static Softdevice);

impl Actor for Nrf52BleMeshTransportActor {
    type Message<'m> = ();
    type OnMountFuture<'m, M>
    where
        Self: 'm,
        M: 'm,
    = impl Future<Output = ()>;

    fn on_mount<'m, M>(&'m mut self, _: Address<Self>, _: &'m mut M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            defmt::info!("start SD");
            self.0.run().await;
            defmt::info!("SD finished?");
        }
    }
}
