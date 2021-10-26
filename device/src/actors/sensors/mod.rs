pub mod hts221;
use crate::domain::{temperature::TemperatureScale, SensorAcquisition};

pub trait SensorMonitor<S: TemperatureScale> {
    fn notify(&self, value: SensorAcquisition<S>);
}
