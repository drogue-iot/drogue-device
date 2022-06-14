use crate::drivers::ble::mesh::bearer::nrf52::Nrf52BleMeshFacilities;
use core::future::Future;
use ector::{Actor, Address, Inbox};

impl Actor for Nrf52BleMeshFacilities {
    type Message<'m> = ();
    type OnMountFuture<'m, M> = impl Future<Output = ()>
    where
        Self: 'm,
        M: 'm + Inbox<()>;

    fn on_mount<'m, M>(&'m mut self, _: Address<()>, _: M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<()> + 'm,
    {
        async move {
            self.sd.run().await;
        }
    }
}
