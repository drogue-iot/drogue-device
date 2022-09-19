//! Simple speaker utilities for PWM-based synth
use embassy_nrf::pwm;

/// Pitch for standard scale
#[allow(dead_code, missing_docs)]
#[derive(Copy, Clone, PartialEq)]
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
    Silent = 0,
}

/// A note is a pitch + a duration
#[derive(Clone, Copy)]
pub struct Note(pub Pitch, pub u32);

/// PWM based speaker capable of playing notes with a given pitch
pub struct PwmSpeaker<'a, T: pwm::Instance> {
    pwm: pwm::SimplePwm<'a, T>,
}

impl<'a, T: pwm::Instance> PwmSpeaker<'a, T> {
    /// Create a new speaker instance
    pub fn new(pwm: pwm::SimplePwm<'a, T>) -> Self {
        Self { pwm }
    }

    /// Play a note
    pub async fn play(&mut self, note: &Note) {
        use embassy_time::{Duration, Timer};
        if note.0 != Pitch::Silent {
            self.pwm.set_prescaler(pwm::Prescaler::Div4);
            self.pwm.set_period(note.0 as u32);
            self.pwm.enable();

            self.pwm.set_duty(0, self.pwm.max_duty() / 2);
            Timer::after(Duration::from_millis(note.1 as u64)).await;
            self.pwm.disable();
        } else {
            Timer::after(Duration::from_millis(note.1 as u64)).await;
        }
    }
}
