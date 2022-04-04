use defmt::Format;

pub mod filter;
pub mod rgb;
pub mod rgbw;

const ONE: u16 = 0x8000 | 13;
// Duty = 13/20 ticks (0.8us/1.25us) for a 1
const ZERO: u16 = 0x8000 | 7;
// Duty 7/20 ticks (0.4us/1.25us) for a 0
const RES: u16 = 0x8000;

pub struct InvalidChannel;

pub trait Pixel<const N: usize>: Copy + Clone + Format {
    const CHANNELS: usize = N;

    fn fill_pwm_words(&self, dst: &mut [u16]) -> Result<(), InvalidChannel> {
        let mut cur = 0;
        for i in 0..Self::CHANNELS {
            let v = self.get(i)?;
            Self::byte_to_word(v, &mut dst[cur..cur + 8]);
            cur += 8;
        }
        Ok(())
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

    fn get(&self, ch: usize) -> Result<u8, InvalidChannel>;
    fn set(&mut self, ch: usize, val: u8) -> Result<(), InvalidChannel>;
}
