use crate::{
    drivers::ble::gatt::dfu::{FirmwareGattService, FirmwareServiceEvent},
    traits::firmware::*,
    Actor, Address, Inbox,
};
use core::future::Future;

impl<'a, F> Actor for FirmwareGattService<'a, F>
where
    F: FirmwareManager,
{
    type Message<'m> = FirmwareServiceEvent;
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<FirmwareServiceEvent>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<FirmwareServiceEvent>,
        mut inbox: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<FirmwareServiceEvent> + 'm,
        Self: 'm,
    {
        async move {
            loop {
                let _ = self.handle(&inbox.next().await).await;
            }
        }
    }
}
