use heapless::Vec;

#[nrf_softdevice::gatt_service(uuid = "181a")]
pub struct EnvironmentSensingService {
    #[characteristic(uuid = "290c", read)]
    pub descriptor: Vec<u8, 11>,

    #[characteristic(uuid = "290d", read)]
    pub trigger: Vec<u8, 4>,

    #[characteristic(uuid = "2a1f", read, notify)]
    pub temperature: i16,

    #[characteristic(uuid = "2a21", read, write)]
    pub period: u16,
}

pub struct MeasurementDescriptor {
    pub flags: u16,
    pub sampling_fn: SamplingFunction,
    pub measurement_period: Period,
    pub update_interval: Interval,
    pub application: MeasurementApp,
    pub uncertainty: Uncertainty,
}

impl Default for MeasurementDescriptor {
    fn default() -> Self {
        Self {
            flags: 0,
            sampling_fn: SamplingFunction::Unspecified,
            measurement_period: Period::Unknown,
            update_interval: Interval::Unknown,
            application: MeasurementApp::Unspecified,
            uncertainty: Uncertainty::Unknown,
        }
    }
}

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum SamplingFunction {
    Unspecified = 0x00,
    Instantaneous = 0x01,
    ArithmeticMean = 0x02,
    RMS = 0x03,
    Max = 0x04,
    Min = 0x05,
    Accum = 0x06,
    Count = 0x07,
}

impl From<u8> for SamplingFunction {
    fn from(value: u8) -> Self {
        match value {
            0x01 => Self::Instantaneous,
            0x02 => Self::ArithmeticMean,
            0x03 => Self::RMS,
            0x04 => Self::Max,
            0x05 => Self::Min,
            0x06 => Self::Accum,
            0x07 => Self::Count,
            _ => Self::Unspecified,
        }
    }
}

#[derive(Copy, Clone)]
pub enum Period {
    Unknown,
    Value(u32),
}

impl Period {
    fn to_gatt(&self) -> [u8; 3] {
        match self {
            Self::Unknown => [0, 0, 0],
            Self::Value(val) => {
                let val = val.to_le_bytes();
                [val[1], val[2], val[3]]
            }
        }
    }
}

impl From<u32> for Period {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Unknown,
            v => Self::Value(v),
        }
    }
}

#[derive(Copy, Clone)]
pub enum Interval {
    Unknown,
    Value(u32),
}

impl Interval {
    fn to_gatt(&self) -> [u8; 3] {
        match self {
            Self::Unknown => [0, 0, 0],
            Self::Value(val) => {
                let val = val.to_le_bytes();
                [val[1], val[2], val[3]]
            }
        }
    }
}

impl From<u32> for Interval {
    fn from(value: u32) -> Self {
        match value {
            0 => Self::Unknown,
            v => Self::Value(v),
        }
    }
}

#[derive(Copy, Clone)]
#[repr(u8)]
pub enum MeasurementApp {
    Unspecified = 0x00,
    Air = 0x01,
    Water = 0x02,
    Barometric = 0x03,
    Soil = 0x04,
    Infrared = 0x05,
    MapDatabase = 0x06,
    BarometricElevation = 0x07,
    GpsElevation = 0x08,
    GpsMapElevation = 0x09,
    VerticalDatumElevation = 0x0A,
    Onshore = 0x0B,
    OnboardVessel = 0x0C,
    Front = 0x0D,
    Back = 0x0E,
    Upper = 0x0F,
    Lower = 0x10,
    Primary = 0x11,
    Secondary = 0x12,
    Outdoor = 0x13,
    Indoor = 0x14,
    Top = 0x15,
    Bottom = 0x16,
    Main = 0x17,
    Backup = 0x18,
    Auxiliary = 0x19,
    Supplementary = 0x1A,
    Inside = 0x1B,
    Outside = 0x1C,
    Left = 0x1D,
    Right = 0x1E,
    Internal = 0x1F,
    External = 0x20,
    Solar = 0x21,
}

impl From<u8> for MeasurementApp {
    fn from(value: u8) -> Self {
        match value {
            0x01 => Self::Air,
            0x02 => Self::Water,
            0x03 => Self::Barometric,
            0x04 => Self::Soil,
            0x05 => Self::Infrared,
            0x06 => Self::MapDatabase,
            0x07 => Self::BarometricElevation,
            0x08 => Self::GpsElevation,
            0x09 => Self::GpsMapElevation,
            0x0A => Self::VerticalDatumElevation,
            0x0B => Self::Onshore,
            0x0C => Self::OnboardVessel,
            0x0D => Self::Front,
            0x0E => Self::Back,
            0x0F => Self::Upper,
            0x10 => Self::Lower,
            0x11 => Self::Primary,
            0x12 => Self::Secondary,
            0x13 => Self::Outdoor,
            0x14 => Self::Indoor,
            0x15 => Self::Top,
            0x16 => Self::Bottom,
            0x17 => Self::Main,
            0x18 => Self::Backup,
            0x19 => Self::Auxiliary,
            0x1A => Self::Supplementary,
            0x1B => Self::Inside,
            0x1C => Self::Outside,
            0x1D => Self::Left,
            0x1E => Self::Right,
            0x1F => Self::Internal,
            0x20 => Self::External,
            0x21 => Self::Solar,
            _ => Self::Unspecified,
        }
    }
}

#[derive(Copy, Clone)]
pub enum Uncertainty {
    Value(u8),
    Unknown,
}

impl Uncertainty {
    fn to_gatt(&self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::Value(val) => *val,
        }
    }
}

impl From<u8> for Uncertainty {
    fn from(value: u8) -> Self {
        match value {
            0 => Self::Unknown,
            v => Self::Value(v),
        }
    }
}

impl MeasurementDescriptor {
    pub fn from_slice(data: &[u8]) -> Self {
        let flags = u16::from_le_bytes([data[0], data[1]]);
        let sampling_fn = data[2].into();
        let measurement_period = u32::from_le_bytes([0, data[3], data[4], data[5]]);
        let measurement_period = if measurement_period == 0 {
            Period::Unknown
        } else {
            Period::Value(measurement_period)
        };

        let update_interval = u32::from_le_bytes([0, data[6], data[7], data[8]]);
        let update_interval = if update_interval == 0 {
            Interval::Unknown
        } else {
            Interval::Value(update_interval)
        };

        let application = data[9].into();
        let uncertainty = data[10].into();

        Self {
            flags,
            sampling_fn,
            measurement_period,
            update_interval,
            application,
            uncertainty,
        }
    }

    pub fn to_vec(&self) -> Vec<u8, 11> {
        let flags = self.flags.to_le_bytes();
        let sampling_fn = self.sampling_fn as u8;
        let measurement_period = self.measurement_period.to_gatt();
        let update_interval = self.update_interval.to_gatt();
        let application = self.application as u8;
        let uncertainty = self.uncertainty.to_gatt();

        Vec::from_slice(&[
            flags[0],
            flags[1],
            sampling_fn,
            measurement_period[0],
            measurement_period[1],
            measurement_period[2],
            update_interval[0],
            update_interval[1],
            update_interval[2],
            application,
            uncertainty,
        ])
        .unwrap()
    }
}

pub enum TriggerSetting {
    Inactive,
    FixedInterval(u32),
    ValueChanged,
}

impl Default for TriggerSetting {
    fn default() -> Self {
        Self::Inactive
    }
}

impl TriggerSetting {
    pub fn from_slice(data: &[u8]) -> Self {
        match data[0] {
            0x01 => {
                let interval = u32::from_le_bytes([0, data[1], data[2], data[3]]);
                Self::FixedInterval(interval)
            }
            0x03 => Self::ValueChanged,
            _ => Self::Inactive,
        }
    }

    pub fn to_vec(&self) -> Vec<u8, 4> {
        match self {
            Self::Inactive => Vec::from_slice(&[0x00]),
            Self::FixedInterval(val) => {
                let val = val.to_le_bytes();
                Vec::from_slice(&[0x02, val[1], val[2], val[3]])
            }
            Self::ValueChanged => Vec::from_slice(&[0x03]),
        }
        .unwrap()
    }
}
