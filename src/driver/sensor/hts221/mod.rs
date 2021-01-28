pub mod package;
pub mod ready;
pub mod register;
pub mod sensor;

pub use package::Hts221;
pub use ready::Ready;
pub use sensor::Sensor;

use crate::domain::temperature::{Temperature, Celsius};
use core::fmt::{Debug, Formatter};

#[derive(Copy, Clone)]
pub struct SensorAcquisition {
    pub temperature: Temperature<Celsius>,
    pub relative_humidity: f32,
}

impl Debug for SensorAcquisition {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("SensorAcquisition")
            .field("temperature", &self.temperature)
            .field("relative_humidity", &self.relative_humidity)
            .finish()
    }
}
