use crate::domain::{temperature::TemperatureScale, SensorAcquisition};
use core::future::Future;

pub trait TemperatureSensor<T: TemperatureScale> {
    type Error;
    type ReadFuture<'m>: Future<Output = Result<SensorAcquisition<T>, Self::Error>>
    where
        Self: 'm;
    fn temperature<'m>(&'m mut self) -> Self::ReadFuture<'m>;
}
