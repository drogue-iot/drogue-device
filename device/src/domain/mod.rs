pub mod led;
pub mod temperature;

use core::fmt::{Debug, Formatter};
use temperature::*;

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

#[cfg(feature = "defmt")]
impl<S: TemperatureScale> defmt::Format for SensorAcquisition<S> {
    fn format(&self, f: defmt::Formatter<'_>) {
        defmt::write!(
            f,
            "SensorAcquisition(temperature: {}, relative_humidity: {})",
            &self.temperature,
            &self.relative_humidity
        );
    }
}
