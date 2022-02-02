use core::future::Future;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::pdu::access::{AccessMessage, AccessPayload};

pub trait AccessContext {
    type DispatchFuture<'m>: Future<Output=Result<(),DeviceError>> + 'm
    where
        Self: 'm;

    fn dispatch_access<'m>(&'m self, message: &'m AccessPayload) -> Self::DispatchFuture<'m>;
}

