use core::num::NonZeroU32;
use nrf_softdevice::{random_bytes, Softdevice};
use rand_core::{CryptoRng, Error, RngCore};

#[derive(Copy, Clone)]
pub struct SoftdeviceRng {
    sd: &'static Softdevice,
}

impl SoftdeviceRng {
    pub fn new(sd: &'static Softdevice) -> Self {
        Self { sd }
    }
}

impl RngCore for SoftdeviceRng {
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0; 4];
        self.fill_bytes(&mut bytes);
        u32::from_be_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0; 8];
        self.fill_bytes(&mut bytes);
        u64::from_be_bytes(bytes)
    }

    fn fill_bytes(&mut self, dest: &mut [u8]) {
        loop {
            match self.try_fill_bytes(dest) {
                Ok(_) => return,
                Err(_) => {
                    // loop again
                }
            }
        }
    }

    fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
        random_bytes(self.sd, dest).map_err(|_| unsafe { NonZeroU32::new_unchecked(99) }.into())
    }
}

impl CryptoRng for SoftdeviceRng {}
