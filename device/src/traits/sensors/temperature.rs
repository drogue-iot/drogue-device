use crate::domain::temperature::{Temperature, TemperatureScale};
use core::future::Future;

pub trait TemperatureSensor<T: TemperatureScale> {
    type Error;
    type ReadFuture<'m>: Future<Output = Result<Temperature<T>, Self::Error>>
    where
        Self: 'm;
    fn temperature<'m>(&'m mut self) -> Self::ReadFuture<'m>;
}
