use crate::drivers::led::neopixel::filter::Filter;
use crate::drivers::led::neopixel::{InvalidChannel, Pixel, RES};
use core::mem::transmute;
use core::ops::Add;
use core::ops::Deref;
use core::slice;
use embassy_executor::time::{Duration, Timer};
use embassy_hal_common::{into_ref, Peripheral};
use embassy_nrf::gpio::Pin;
use embassy_nrf::pwm::{
    Config, Error, Instance, Prescaler, SequenceConfig, SequenceLoad, SequencePwm,
    SingleSequenceMode, SingleSequencer,
};

pub const BLACK: Rgbw8 = Rgbw8::new(0x00, 0x00, 0x00, 0x00);
pub const WHITE: Rgbw8 = Rgbw8::new(0x00, 0x00, 0x000, 0xFF);
pub const RED: Rgbw8 = Rgbw8::new(0xFF, 0x00, 0x00, 0x00);
pub const GREEN: Rgbw8 = Rgbw8::new(0x00, 0xFF, 0x00, 0x00);
pub const BLUE: Rgbw8 = Rgbw8::new(0x00, 0x00, 0xFF, 0x00);

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Rgbw8 {
    r: u8,
    g: u8,
    b: u8,
    w: u8,
}

impl Pixel<4> for Rgbw8 {
    fn get(&self, ch: usize) -> Result<u8, InvalidChannel> {
        match ch {
            0 => Ok(self.g),
            1 => Ok(self.r),
            2 => Ok(self.b),
            3 => Ok(self.w),
            _ => Err(InvalidChannel),
        }
    }

    fn set(&mut self, ch: usize, val: u8) -> Result<(), InvalidChannel> {
        match ch {
            0 => self.g = val,
            1 => self.r = val,
            2 => self.b = val,
            3 => self.w = val,
            _ => Err(InvalidChannel)?,
        }
        Ok(())
    }
}

impl Rgbw8 {
    pub const fn new(r: u8, g: u8, b: u8, w: u8) -> Self {
        Self { r, g, b, w }
    }
}

impl Add for Rgbw8 {
    type Output = Rgbw8;

    fn add(self, rhs: Self) -> Self::Output {
        Self::Output {
            r: self.r.saturating_add(rhs.r),
            g: self.g.saturating_add(rhs.g),
            b: self.b.saturating_add(rhs.b),
            w: self.w.saturating_add(rhs.w),
        }
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(C)]
struct RawPwmRgbw8<const N: usize> {
    words: [[u16; 32]; N],
    end: [u16; 40],
}

impl<const N: usize> RawPwmRgbw8<N> {
    pub fn from_iter<'i, I: Iterator<Item = &'i Rgbw8>>(iter: I) -> Result<Self, Error> {
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

impl<const N: usize> Default for RawPwmRgbw8<N> {
    fn default() -> Self {
        Self {
            words: [[0; 32]; N],
            end: [RES; 40],
        }
    }
}

impl<const N: usize> Into<RawPwmRgbw8<N>> for &[Rgbw8; N] {
    fn into(self) -> RawPwmRgbw8<N> {
        let mut raw = RawPwmRgbw8::default();
        let mut cur = 0;
        for color in self {
            color.fill_pwm_words(&mut raw.words[cur]).ok().unwrap();
            cur += 1;
        }
        raw
    }
}

impl<const N: usize> Deref for RawPwmRgbw8<N> {
    type Target = [u16];

    fn deref(&self) -> &Self::Target {
        unsafe {
            let ptr: *const u16 = transmute(self as *const _ as *const u16);
            slice::from_raw_parts(ptr, (N * 32) + 40)
        }
    }
}

pub struct NeoPixelRgbw<'d, T: Instance, const N: usize = 1> {
    pwm: SequencePwm<'d, T>,
}

impl<'d, T: Instance, const N: usize> NeoPixelRgbw<'d, T, N> {
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

    pub async fn set(&mut self, pixels: &[Rgbw8; N]) -> Result<(), Error> {
        let mut seq_config = SequenceConfig::default();
        seq_config.end_delay = 799;

        let raw: RawPwmRgbw8<N> = pixels.into();
        let raw = &*raw;

        let sequences = SingleSequencer::new(&mut self.pwm, &*raw, seq_config);
        sequences.start(SingleSequenceMode::Times(1))?;

        Timer::after(Duration::from_micros((30 * (N as u64 + 40)) + 100)).await;
        Ok(())
    }

    pub async fn set_from_iter<'i, I: Iterator<Item = &'i Rgbw8>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Error> {
        let mut seq_config = SequenceConfig::default();
        seq_config.end_delay = 799;

        //let raw: RawPwmRgbw8<N> = pixels.into();
        let raw = RawPwmRgbw8::<N>::from_iter(pixels)?;
        let raw = &*raw;

        let sequences = SingleSequencer::new(&mut self.pwm, &*raw, seq_config);
        sequences.start(SingleSequenceMode::Times(1))?;

        Timer::after(Duration::from_micros((30 * (N as u64 + 40)) + 100)).await;
        Ok(())
    }

    pub async fn set_with_filter<F: Filter<Rgbw8, 4>>(
        &mut self,
        pixels: &[Rgbw8; N],
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
