use drogue_device::traits::led::LedMatrix;

use embassy::time::{Duration, Instant, Timer};
use embassy_nrf::pwm::*;

pub struct Sample<'a> {
    notes: &'a [Note],
}

impl<'a> Sample<'a> {
    pub const fn new(notes: &'a [Note]) -> Self {
        Self { notes }
    }
}

pub enum Pitch {
    C = 261,
    D = 293,
    E = 329,
    F = 349,
    G = 391,
    A = 440,
    AB = 466,
    B = 493,
    C2 = 523,
}

#[derive(Clone, Copy)]
pub struct Note(pub u32, pub u32);

pub struct PwmSpeaker<'a, T: Instance, M: LedMatrix<5, 5>> {
    pwm: SimplePwm<'a, T>,
    matrix: M,
}

impl<'a, T: Instance, M: LedMatrix<5, 5>> PwmSpeaker<'a, T, M> {
    pub fn new(pwm: SimplePwm<'a, T>, matrix: M) -> Self {
        Self { pwm, matrix }
    }

    pub async fn play_note(&mut self, note: Note) {
        if note.0 > 0 {
            self.pwm.set_prescaler(Prescaler::Div4);
            self.pwm.set_period(note.0);
            self.pwm.enable();

            self.pwm.set_duty(0, self.pwm.max_duty() / 2);
            let _ = self.matrix.apply(&BITMAP).await;
            let _ = self.matrix.max_brightness();
            let interval = Duration::from_millis(note.1 as u64 / 10);
            let end = Instant::now() + Duration::from_millis(note.1 as u64);
            while Instant::now() < end {
                let _ = self.matrix.decrease_brightness();
                Timer::after(interval).await;
            }
            let _ = self.matrix.clear().await;
            self.pwm.disable();
        } else {
            Timer::after(Duration::from_millis(note.1 as u64)).await;
        }
    }

    pub async fn play_sample(&mut self, sample: &Sample<'_>) {
        for note in sample.notes.iter() {
            self.play_note(*note).await;
        }
    }
}

pub const BITMAP: &[u8; 5] = &[0b11111, 0b11111, 0b11111, 0b11111, 0b11111];
