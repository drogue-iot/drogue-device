use core::mem::transmute;
use core::ops::Add;
use core::ops::Deref;
use core::ptr;
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

// This table remaps linear input values
// (the numbers we’d like to use; e.g. 127 = half brightness)
// to nonlinear gamma-corrected output values
// (numbers producing the desired effect on the LED;
// e.g. 36 = half brightness).
const GAMMA8: [u8; 256] = [
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 5, 5, 5,
    5, 6, 6, 6, 6, 7, 7, 7, 7, 8, 8, 8, 9, 9, 9, 10, 10, 10, 11, 11, 11, 12, 12, 13, 13, 13, 14,
    14, 15, 15, 16, 16, 17, 17, 18, 18, 19, 19, 20, 20, 21, 21, 22, 22, 23, 24, 24, 25, 25, 26, 27,
    27, 28, 29, 29, 30, 31, 32, 32, 33, 34, 35, 35, 36, 37, 38, 39, 39, 40, 41, 42, 43, 44, 45, 46,
    47, 48, 49, 50, 50, 51, 52, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 66, 67, 68, 69, 70, 72,
    73, 74, 75, 77, 78, 79, 81, 82, 83, 85, 86, 87, 89, 90, 92, 93, 95, 96, 98, 99, 101, 102, 104,
    105, 107, 109, 110, 112, 114, 115, 117, 119, 120, 122, 124, 126, 127, 129, 131, 133, 135, 137,
    138, 140, 142, 144, 146, 148, 150, 152, 154, 156, 158, 160, 162, 164, 167, 169, 171, 173, 175,
    177, 180, 182, 184, 186, 189, 191, 193, 196, 198, 200, 203, 205, 208, 210, 213, 215, 218, 220,
    223, 225, 228, 231, 233, 236, 239, 241, 244, 247, 249, 252, 255,
];

pub trait Filter {
    fn apply(&mut self, pixel: &Rgb8) -> Rgb8;

    fn and<F: Filter + Sized>(self, filter: F) -> ComposedFilter<Self, F>
    where
        Self: Sized,
    {
        ComposedFilter::new(self, filter)
    }
}

pub struct Gamma;

impl Filter for Gamma {
    fn apply(&mut self, pixel: &Rgb8) -> Rgb8 {
        Rgb8::new(
            GAMMA8[pixel.r as usize],
            GAMMA8[pixel.g as usize],
            GAMMA8[pixel.b as usize],
        )
    }
}

pub struct Brightness(pub u8);

impl Brightness {
    fn percent(mut percent: u8) -> Self {
        if percent > 100 {
            percent = 100;
        }

        if percent == 0 {
            Self(0)
        } else {
            Self((255 / percent) as u8)
        }
    }
}

impl Filter for Brightness {
    fn apply(&mut self, pixel: &Rgb8) -> Rgb8 {
        Rgb8::new(
            (pixel.r as u16 * (self.0 as u16 + 1) / 256) as u8,
            (pixel.g as u16 * (self.0 as u16 + 1) / 256) as u8,
            (pixel.b as u16 * (self.0 as u16 + 1) / 256) as u8,
        )
    }
}

pub enum CyclicDirection {
    Up,
    Down,
}

pub struct CyclicBrightness {
    low: u8,
    high: u8,
    current: u8,
    direction: CyclicDirection,
    step_size: u8,
    completed_cycles: u32,
}

impl CyclicBrightness {
    pub fn new(low: u8, high: u8, step_size: u8) -> Self {
        Self {
            low,
            high,
            current: low,
            direction: CyclicDirection::Up,
            step_size,
            completed_cycles: 0,
        }
    }
}

impl Filter for CyclicBrightness {
    fn apply(&mut self, pixel: &Rgb8) -> Rgb8 {
        let pixel = Brightness(self.current).apply(pixel);
        match self.direction {
            CyclicDirection::Up => {
                if self.current.saturating_add(self.step_size) >= self.high {
                    self.current = self.high;
                    self.direction = CyclicDirection::Down;
                } else {
                    self.current += self.step_size;
                }
            }
            CyclicDirection::Down => {
                if self.current.saturating_sub(self.step_size) <= self.low {
                    self.current = self.low;
                    self.direction = CyclicDirection::Up;
                    self.completed_cycles += 1;
                } else {
                    self.current -= self.step_size;
                }
            }
        }
        pixel
    }
}

pub struct ComposedFilter<F1: Filter + Sized, F2: Filter + Sized> {
    f1: F1,
    f2: F2,
}

impl<F1: Filter + Sized, F2: Filter + Sized> ComposedFilter<F1, F2> {
    fn new(f1: F1, f2: F2) -> Self {
        Self { f1, f2 }
    }
}

impl<F1: Filter, F2: Filter> Filter for ComposedFilter<F1, F2> {
    fn apply(&mut self, pixel: &Rgb8) -> Rgb8 {
        let pixel = self.f1.apply(pixel);
        self.f2.apply(&pixel)
    }
}

pub const BLACK: Rgb8 = Rgb8::new(0x00, 0x00, 0x00);
pub const WHITE: Rgb8 = Rgb8::new(0xFF, 0xFF, 0x0FF);
pub const RED: Rgb8 = Rgb8::new(0xFF, 0x00, 0x00);
pub const GREEN: Rgb8 = Rgb8::new(0x00, 0xFF, 0x00);
pub const BLUE: Rgb8 = Rgb8::new(0x00, 0x00, 0xFF);

#[derive(Copy, Clone, Format, PartialEq, Eq)]
pub struct Rgb8 {
    r: u8,
    g: u8,
    b: u8,
}

const ONE: u16 = 0x8000 | 13; // Duty = 13/20 ticks (0.8us/1.25us) for a 1
const ZERO: u16 = 0x8000 | 7; // Duty 7/20 ticks (0.4us/1.25us) for a 0
const RES: u16 = 0x8000;

impl Rgb8 {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
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

    pub async fn set(&mut self, pixels: &[Rgb8; N]) -> Result<(), Error> {
        let mut seq_config = SequenceConfig::default();
        seq_config.end_delay = 799;

        let raw: Raw<N> = pixels.into();
        let raw = &*raw;

        let event = self.pwm.event_seq_end();

        let sequences = SingleSequencer::new(&mut self.pwm, &*raw, seq_config);
        sequences.start(SingleSequenceMode::Times(1))?;

        Timer::after(Duration::from_micros((30 * (N as u64 + 40)) + 100)).await;
        Ok(())
    }

    pub async fn set_with_filter<F: Filter>(
        &mut self,
        pixels: &[Rgb8; N],
        filter: &mut F,
    ) -> Result<(), Error> {
        let mut filtered = [BLACK; N];
        for (i, pixel) in pixels.iter().enumerate() {
            filtered[i] = filter.apply(pixel);
        }
        self.set(&filtered).await
    }
}
