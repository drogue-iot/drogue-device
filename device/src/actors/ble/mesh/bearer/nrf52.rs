use crate::drivers::ble::mesh::bearer::nrf52::Nrf52BleMeshFacilities;
use crate::{Actor, Address, Inbox};
use core::future::Future;

impl Actor for Nrf52BleMeshFacilities {
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
