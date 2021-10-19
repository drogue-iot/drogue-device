use embassy::time::Duration;

pub struct TemperatureMonitor<'d> {
    t: Temp<'d>,
    interval: Duration,
}

impl<'d> Actor for TemperatureMonitor {}
