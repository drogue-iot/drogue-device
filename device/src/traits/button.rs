use core::future::Future;

pub trait Button {
    type WaitPressed<'m>: Future<Output = ()>
    where
        Self: 'm;

    type WaitReleased<'m>: Future<Output = ()>
    where
        Self: 'm;

    fn wait_pressed<'m>(&'m mut self) -> Self::WaitPressed<'m>
    where
        Self: 'm;

    fn wait_released<'m>(&'m mut self) -> Self::WaitReleased<'m>
    where
        Self: 'm;
}
