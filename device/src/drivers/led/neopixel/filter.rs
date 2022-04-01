use crate::drivers::led::neopixel::{InvalidChannel, Pixel};
use core::marker::PhantomData;

pub struct ComposedFilter<
    P: Pixel<C>,
    F1: Filter<P, C> + Sized,
    F2: Filter<P, C> + Sized,
    const C: usize,
> {
    f1: F1,
    f2: F2,
    _marker: PhantomData<P>,
}

impl<P: Pixel<C>, F1: Filter<P, C> + Sized, F2: Filter<P, C> + Sized, const C: usize>
    ComposedFilter<P, F1, F2, C>
{
    fn new(f1: F1, f2: F2) -> Self {
        Self {
            f1,
            f2,
            _marker: PhantomData,
        }
    }
}

impl<P: Pixel<C>, F1: Filter<P, C>, F2: Filter<P, C>, const C: usize> Filter<P, C>
    for ComposedFilter<P, F1, F2, C>
{
    fn apply(&self, pixel: &P) -> Result<P, InvalidChannel> {
        let pixel = self.f1.apply(pixel)?;
        self.f2.apply(&pixel)
    }

    fn complete(&mut self) {
        self.f1.complete();
        self.f2.complete();
    }
}

pub trait Filter<P: Pixel<C>, const C: usize> {
    fn apply(&self, pixel: &P) -> Result<P, InvalidChannel>;

    fn complete(&mut self) {}

    fn and<F: Filter<P, C> + Sized>(self, filter: F) -> ComposedFilter<P, Self, F, C>
    where
        Self: Sized,
    {
        ComposedFilter::new(self, filter)
    }
}

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

pub struct Gamma;

impl<P: Pixel<C>, const C: usize> Filter<P, C> for Gamma {
    fn apply(&self, pixel: &P) -> Result<P, InvalidChannel> {
        let mut filtered = pixel.clone();
        for i in 0..C {
            filtered.set(i, GAMMA8[pixel.get(i)? as usize])?;
        }
        Ok(filtered)
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

impl<P: Pixel<C>, const C: usize> Filter<P, C> for Brightness {
    fn apply(&self, pixel: &P) -> Result<P, InvalidChannel> {
        let mut filtered = pixel.clone();
        for i in 0..C {
            let val = pixel.get(i)?;
            let _filtered_val = (val as u16 * (self.0 as u16 + 1) / 256) as u8;
            filtered.set(i, (pixel.get(i)? as u16 * (self.0 as u16 + 1) / 256) as u8)?;
        }
        Ok(filtered)
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
}

impl CyclicBrightness {
    pub fn new(low: u8, high: u8, step_size: u8) -> Self {
        Self {
            low,
            high,
            current: low,
            direction: CyclicDirection::Up,
            step_size,
        }
    }
}

impl<P: Pixel<C>, const C: usize> Filter<P, C> for CyclicBrightness {
    fn apply(&self, pixel: &P) -> Result<P, InvalidChannel> {
        Brightness(self.current).apply(pixel)
    }

    fn complete(&mut self) {
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
                } else {
                    self.current -= self.step_size;
                }
            }
        }
    }
}
