use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use core::future::Future;

pub trait AccessContext {
    type DispatchFuture<'m>: Future<Output = Result<(), DeviceError>> + 'm
    where
        Self: 'm;

    fn dispatch_access<'m>(&'m self, message: &'m AccessMessage) -> Self::DispatchFuture<'m>;
}
