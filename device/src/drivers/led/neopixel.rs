use core::mem::transmute;
use core::ops::Deref;
use core::slice;
use defmt::Format;
use embassy::time::{Duration, Timer};
use embassy::util::Unborrow;
use embassy_hal_common::unborrow;
use embassy_nrf::gpio::Pin;
use embassy_nrf::pwm::{
    Config, Error, Instance, Prescaler, SequenceConfig, SequenceLoad, SequencePwm,
    SingleSequenceMode, SingleSequencer,
};

#[derive(Copy, Clone, Format, PartialEq, Eq)]
pub struct Rgb8 {
    r: u8,
    g: u8,
    b: u8,
}

pub const RED: Rgb8 = Rgb8::new(0xFF, 0x00, 0x00);
pub const GREEN: Rgb8 = Rgb8::new(0x00, 0xFF, 0x00);
pub const BLUE: Rgb8 = Rgb8::new(0x00, 0x00, 0xFF);

const ONE: u16 = 0x8000 | 13; // Duty = 13/20 ticks (0.8us/1.25us) for a 1
const ZERO: u16 = 0x8000 | 7; // Duty 7/20 ticks (0.4us/1.25us) for a 0
const RES: u16 = 0x8000;

impl Rgb8 {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    pub fn scale(&self, factor: f32) -> Self {
        let r = ((self.r as f32) * factor) as u8;
        let g = ((self.g as f32) * factor) as u8;
        let b = ((self.b as f32) * factor) as u8;

        Self { r, g, b }
    }

    fn byte_to_word(byte: u8, dst: &mut [u16]) {
        let mut pos = 0;
        let mut mask = 0x80;
        for _ in 0..8 {
            if (byte & mask) != 0 {
                dst[pos] = ONE;
            } else {
                dst[pos] = ZERO;
            }
            pos += 1;
            mask >>= 1;
        }
    }

    fn to_pwm_words(&self, dst: &mut [u16]) {
        Self::byte_to_word(self.g, &mut dst[0..8]);
        Self::byte_to_word(self.r, &mut dst[8..16]);
        Self::byte_to_word(self.b, &mut dst[16..24]);
    }
}

#[repr(C)]
struct Raw<const N: usize> {
    words: [[u16; 24]; N],
    end: [u16; 40],
}

impl<const N: usize> Default for Raw<N> {
    fn default() -> Self {
        Self {
            words: [[0; 24]; N],
            end: [RES; 40],
        }
    }
}

impl<const N: usize> Into<Raw<N>> for &[Rgb8; N] {
    fn into(self) -> Raw<N> {
        let mut raw = Raw::default();
        let mut cur = 0;
        for color in self {
            color.to_pwm_words(&mut raw.words[cur]);
            cur += 1;
        }
        raw
    }
}

impl<const N: usize> Deref for Raw<N> {
    type Target = [u16];

    fn deref(&self) -> &Self::Target {
        unsafe {
            let ptr: *const u16 = transmute(self.words.as_ptr() as *const u16);
            slice::from_raw_parts(ptr, (N * 24) + 40)
        }
    }
}

pub struct NeoPixel<'d, T: Instance, const N: usize = 1> {
    pwm: SequencePwm<'d, T>,
}

impl<'d, T: Instance, const N: usize> NeoPixel<'d, T, N> {
    pub fn new(
        pwm: impl Unborrow<Target = T>,
        pin: impl Unborrow<Target = impl Pin> + 'd,
    ) -> Result<Self, Error> {
        unborrow!(pwm);
        unborrow!(pin);
        let mut config = Config::default();
        config.sequence_load = SequenceLoad::Common;
        config.prescaler = Prescaler::Div1;
        config.max_duty = 20; // 1.25us (1s / 16Mhz * 20)

        Ok(Self {
            pwm: SequencePwm::new_1ch(pwm, pin, config)?,
        })
    }

    pub async fn set(&mut self, color: &[Rgb8; N]) -> Result<(), Error> {
        let mut seq_config = SequenceConfig::default();
        seq_config.end_delay = 799;

        let raw: Raw<N> = color.into();
        let raw = &*raw;

        let sequences = SingleSequencer::new(&mut self.pwm, &*raw, seq_config);
        sequences.start(SingleSequenceMode::Times(1))?;

        Timer::after(Duration::from_millis(1)).await;
        Ok(())
    }
}
