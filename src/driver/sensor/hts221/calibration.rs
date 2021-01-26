

pub struct Calibration {
    pub temperature: Option<TemperatureCalibration>,
    pub humidity: Option<HumidityCalibration>,
}

pub struct TemperatureCalibration {
    pub t0_out: i16,
    pub t1_out: i16,
    pub t0_degc: f32,
    pub t1_degc: f32,
    pub slope: f32,
}

pub struct HumidityCalibration {
    pub h0_out: i16,
    pub h1_out: i16,
    pub h0_rh: f32,
    pub h1_rh: f32,
    pub slope: f32
}

impl Calibration {
    pub fn new() -> Self {
        Self {
            temperature: None,
            humidity: None,
        }
    }
}