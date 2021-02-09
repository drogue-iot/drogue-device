pub mod package;
pub mod ready;
pub mod register;
pub mod sensor;

pub use package::Hts221;
pub use ready::Ready;
pub use sensor::Sensor;

use crate::domain::temperature::{Temperature, TemperatureScale};
use core::fmt::{Debug, Formatter};

#[derive(Copy, Clone)]
pub struct SensorAcquisition<S: TemperatureScale> {
    pub temperature: Temperature<S>,
    pub relative_humidity: f32,
}

impl<S: TemperatureScale> Debug for SensorAcquisition<S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SensorAcquisition")
            .field("temperature", &self.temperature)
            .field("relative_humidity", &self.relative_humidity)
            .finish()
    }
}
