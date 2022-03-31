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
    type Message<'m> = FirmwareServiceEvent where Self: 'm;
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<Self>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
        Self: 'm,
    {
        async move {
            loop {
                if let Some(mut m) = inbox.next().await {
                    let _ = self.handle(m.message()).await;
                }
            }
        }
    }
}
