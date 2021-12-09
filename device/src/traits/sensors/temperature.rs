use crate::domain::{temperature::TemperatureScale, SensorAcquisition};
use core::future::Future;

pub trait TemperatureSensor<T: TemperatureScale> {
    type Error: Send;
    type CalibrateFuture<'m>: Future<Output = Result<(), Self::Error>>
    where
        Self: 'm;
    fn calibrate<'m>(&'m mut self) -> Self::CalibrateFuture<'m>;

    type ReadFuture<'m>: Future<Output = Result<SensorAcquisition<T>, Self::Error>>
    where
        Self: 'm;
    fn temperature<'m>(&'m mut self) -> Self::ReadFuture<'m>;
}
