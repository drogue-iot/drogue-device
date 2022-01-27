use crate::domain::temperature::{Celsius, Temperature};
use crate::traits::i2c::I2cAddress;
use embedded_hal_async::i2c::*;

// 16-byte block of calibration at 0x30 with high bit for auto-increment
const CALIBRATION_16: u8 = 0xB0;

pub struct Calibration {
    pub temperature: TemperatureCalibration,
    pub humidity: HumidityCalibration,
}

impl Calibration {
    pub async fn read<I: I2c>(address: I2cAddress, i2c: &mut I) -> Result<Calibration, I::Error> {
        let mut buf = [0; 16];
        let _ = i2c
            .write_read(address.into(), &[CALIBRATION_16], &mut buf)
            .await?;
        Ok(buf.into())
    }

    pub fn calibrated_temperature(&self, t_out: i16) -> Temperature<Celsius> {
        self.temperature.calibrated(t_out)
    }

    pub fn calibrated_humidity(&self, h_out: i16) -> f32 {
        self.humidity.calibrated(h_out)
    }
}

pub struct TemperatureCalibration {
    pub t0_out: i16,
    pub t1_out: i16,
    pub t0_degc: Temperature<Celsius>,
    pub t1_degc: Temperature<Celsius>,
    pub slope: f32,
}

impl TemperatureCalibration {
    pub fn calibrated(&self, t_out: i16) -> Temperature<Celsius> {
        self.t0_degc + (self.slope * (t_out - self.t0_out) as f32)
    }
}

pub struct HumidityCalibration {
    pub h0_out: i16,
    pub h1_out: i16,
    pub h0_rh: f32,
    pub h1_rh: f32,
    pub slope: f32,
}

impl HumidityCalibration {
    pub fn calibrated(&self, h_out: i16) -> f32 {
        self.h0_rh + (self.slope * (h_out - self.h0_out) as f32)
    }
}

impl Into<Calibration> for [u8; 16] {
    fn into(self) -> Calibration {
        let t0_out = i16::from_le_bytes([self[12], self[13]]);

        let t1_out = i16::from_le_bytes([self[14], self[15]]);

        let t0_degc = self[2];
        let t1_degc = self[3];

        let t_msb = self[5];

        let t0_msb = (t_msb & 0b00000011) >> 2;
        let t1_msb = (t_msb & 0b00001100) >> 2;

        let t0_degc = (i16::from_le_bytes([t0_degc, t0_msb]) as f32 / 8.0).into();
        let t1_degc = (i16::from_le_bytes([t1_degc, t1_msb]) as f32 / 8.0).into();

        let slope = (t1_degc - t0_degc) / ((t1_out - t0_out) as f32);

        let temperature = TemperatureCalibration {
            t0_out,
            t1_out,
            t0_degc,
            t1_degc,
            slope,
        };

        let h0_rh = self[0] as f32 / 2.0;
        let h1_rh = self[1] as f32 / 2.0;

        let h0_out = i16::from_le_bytes([self[6], self[7]]);

        let h1_out = i16::from_le_bytes([self[10], self[11]]);

        let slope = (h1_rh - h0_rh) / ((h1_out - h0_out) as f32);

        let humidity = HumidityCalibration {
            h0_out,
            h1_out,
            h0_rh,
            h1_rh,
            slope,
        };

        Calibration {
            temperature,
            humidity,
        }
    }
}
