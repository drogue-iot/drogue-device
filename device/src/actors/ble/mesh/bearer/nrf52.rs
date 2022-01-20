use crate::drivers::ble::mesh::bearer::nrf52::Nrf52BleMeshFacilities;
use crate::{Actor, Address, Inbox};
use core::future::Future;
use nrf_softdevice::Softdevice;

impl Actor for Nrf52BleMeshFacilities {
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
            self.sd.run().await;
            defmt::info!("SD finished?");
        }
    }
}
