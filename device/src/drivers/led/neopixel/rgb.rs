use crate::drivers::led::neopixel::filter::Filter;
use crate::drivers::led::neopixel::{InvalidChannel, Pixel, RES};
use core::mem::transmute;
use core::ops::Add;
use core::ops::Deref;
use core::slice;
use embassy_hal_common::{into_ref, Peripheral};
use embassy_nrf::gpio::Pin;
use embassy_nrf::pwm::{
    Config, Error, Instance, Prescaler, SequenceConfig, SequenceLoad, SequencePwm,
    SingleSequenceMode, SingleSequencer,
};
use embassy_time::{Duration, Timer};

pub const BLACK: Rgb8 = Rgb8::new(0x00, 0x00, 0x00);
pub const WHITE: Rgb8 = Rgb8::new(0xFF, 0xFF, 0x0FF);
pub const RED: Rgb8 = Rgb8::new(0xFF, 0x00, 0x00);
pub const GREEN: Rgb8 = Rgb8::new(0x00, 0xFF, 0x00);
pub const BLUE: Rgb8 = Rgb8::new(0x00, 0x00, 0xFF);

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Rgb8 {
    r: u8,
    g: u8,
    b: u8,
}

impl Pixel<3> for Rgb8 {
    fn get(&self, ch: usize) -> Result<u8, InvalidChannel> {
        match ch {
            0 => Ok(self.g),
            1 => Ok(self.r),
            2 => Ok(self.b),
            _ => Err(InvalidChannel),
        }
    }

    fn set(&mut self, ch: usize, val: u8) -> Result<(), InvalidChannel> {
        match ch {
            0 => self.g = val,
            1 => self.r = val,
            2 => self.b = val,
            _ => Err(InvalidChannel)?,
        }
        Ok(())
    }
}

impl Rgb8 {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

impl Add for Rgb8 {
    type Output = Rgb8;

    fn add(self, rhs: Self) -> Self::Output {
        Self::Output {
            r: self.r.saturating_add(rhs.r),
            g: self.g.saturating_add(rhs.g),
            b: self.b.saturating_add(rhs.b),
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(C)]
struct RawPwmRgb8<const N: usize> {
    words: [[u16; 24]; N],
    end: [u16; 40],
}

impl<const N: usize> RawPwmRgb8<N> {
    pub fn from_iter<'i, I: Iterator<Item = &'i Rgb8>>(iter: I) -> Result<Self, Error> {
        let mut raw = Self::default();
        let mut cur = 0;
        for color in iter {
            if cur > N {
                return Err(Error::SequenceTooLong);
            }
            color.fill_pwm_words(&mut raw.words[cur]).ok().unwrap();
            cur += 1;
        }
        Ok(raw)
    }
}

impl<const N: usize> Default for RawPwmRgb8<N> {
    fn default() -> Self {
        Self {
            words: [[0; 24]; N],
            end: [RES; 40],
        }
    }
}

impl<const N: usize> Into<RawPwmRgb8<N>> for &[Rgb8; N] {
    fn into(self) -> RawPwmRgb8<N> {
        let mut raw = RawPwmRgb8::default();
        let mut cur = 0;
        for color in self {
            color.fill_pwm_words(&mut raw.words[cur]).ok().unwrap();
            cur += 1;
        }
        raw
    }
}

impl<const N: usize> Deref for RawPwmRgb8<N> {
    type Target = [u16];

    fn deref(&self) -> &Self::Target {
        unsafe {
            let ptr: *const u16 = transmute(self as *const _ as *const u16);
            slice::from_raw_parts(ptr, (N * 24) + 40)
        }
    }
}

pub struct NeoPixelRgb<'d, T: Instance, const N: usize = 1> {
    pwm: SequencePwm<'d, T>,
}

impl<'d, T: Instance, const N: usize> NeoPixelRgb<'d, T, N> {
    pub fn new(
        pwm: impl Peripheral<P = T> + 'd,
        pin: impl Peripheral<P = impl Pin> + 'd,
    ) -> Result<Self, Error> {
        into_ref!(pwm);
        into_ref!(pin);
        let mut config = Config::default();
        config.sequence_load = SequenceLoad::Common;
        config.prescaler = Prescaler::Div1;
        config.max_duty = 20; // 1.25us (1s / 16Mhz * 20)

        Ok(Self {
            pwm: SequencePwm::new_1ch(pwm, pin, config)?,
        })
    }

    pub async fn set(&mut self, pixels: &[Rgb8; N]) -> Result<(), Error> {
        let mut seq_config = SequenceConfig::default();
        seq_config.end_delay = 799;

        let raw: RawPwmRgb8<N> = pixels.into();
        let raw = &*raw;

        let sequences = SingleSequencer::new(&mut self.pwm, &*raw, seq_config);
        sequences.start(SingleSequenceMode::Times(1))?;

        Timer::after(Duration::from_micros((30 * (N as u64 + 40)) + 100)).await;
        Ok(())
    }

    pub async fn set_from_iter<'i, I: Iterator<Item = &'i Rgb8>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Error> {
        let mut seq_config = SequenceConfig::default();
        seq_config.end_delay = 799;

        let raw = RawPwmRgb8::<N>::from_iter(pixels)?;
        let raw = &*raw;

        let sequences = SingleSequencer::new(&mut self.pwm, &*raw, seq_config);
        sequences.start(SingleSequenceMode::Times(1))?;

        Timer::after(Duration::from_micros((30 * (N as u64 + 40)) + 100)).await;
        Ok(())
    }

    pub async fn set_with_filter<F: Filter<Rgb8, 3>>(
        &mut self,
        pixels: &[Rgb8; N],
        filter: &mut F,
    ) -> Result<(), Error> {
        let mut filtered = [BLACK; N];
        for (i, pixel) in pixels.iter().enumerate() {
            filtered[i] = filter
                .apply(pixel)
                .map_err(|_| Error::SequenceTimesAtLeastOne)?;
        }
        filter.complete();
        self.set(&filtered).await
    }
}
