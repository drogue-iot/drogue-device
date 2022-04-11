use crate::{
    drivers::ble::gatt::dfu::{FirmwareGattService, FirmwareServiceEvent},
    Actor, Address, Inbox,
};
use core::future::Future;
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};

impl<'a, F> Actor for FirmwareGattService<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
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
