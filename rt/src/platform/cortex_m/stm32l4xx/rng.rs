use rand_core::{CryptoRng, Error, RngCore};
use stm32l4xx_hal::rng::Rng as HalRng;

static mut RANDOM: Option<HalRng> = None;

#[derive(Copy, Clone)]
pub struct Random {}

impl Random {
    pub fn initialize(rng: HalRng) -> Self {
        unsafe {
            if RANDOM.is_none() {
                RANDOM.replace(rng);
            }
        }

        Random {}
    }
}

impl CryptoRng for Random {}

impl RngCore for Random {
    fn next_u32(&mut self) -> u32 {
        unsafe { RANDOM.as_mut().unwrap().get_random_data() }
    }

    fn next_u64(&mut self) -> u64 {
        unsafe {
            let a = RANDOM.as_mut().unwrap().get_random_data();
            let b = RANDOM.as_mut().unwrap().get_random_data();
            (a as u64) << 32 + b
        }
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        let mut data = 0;

        for (index, slot) in dest.iter_mut().enumerate() {
            if index % 4 == 0 {
                data = self.next_u32();
            }

            *slot = data as u8 & 0xff;
            data = data >> 8;
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        self.fill_bytes(dest);
        Ok(())
    }
}
