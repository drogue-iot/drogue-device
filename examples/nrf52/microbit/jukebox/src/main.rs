#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use drogue_device::{
    bsp::boards::nrf52::microbit::*, domain::led::matrix::Brightness, traits::led::ToFrame, Board,
};

use embassy_nrf::{pwm::*, Peripherals};

use embassy_executor::time::{Duration, Instant, Timer};
use futures::future::join;

use panic_probe as _;

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::executor::Spawner, p: Peripherals) {
    let board = Microbit::new(p);
    let mut disco = Disco::new(board.display);

    let pwm = SimplePwm::new_1ch(board.pwm0, board.speaker);
    let mut speaker = PwmSpeaker::new(pwm);

    loop {
        for i in 0..RIFF.len() {
            join(disco.flash(RIFF[i]), speaker.play_note(RIFF[i])).await;
        }
    }
}

pub struct Disco {
    display: LedMatrix,
}

impl Disco {
    pub fn new(display: LedMatrix) -> Self {
        Self { display }
    }

    pub async fn flash(&mut self, note: Note) {
        if note.0 != Pitch::Silent {
            let f = BITMAP.to_frame();
            self.display.apply(f);
            self.display.set_brightness(Brightness::MAX);

            let interval = Duration::from_millis(note.1 as u64 / 10);
            let end = Instant::now() + Duration::from_millis(note.1 as u64);
            while Instant::now() < end {
                let _ = self.display.decrease_brightness();
                self.display.display(f, interval).await;
            }
            self.display.clear();
        } else {
            Timer::after(Duration::from_millis(note.1 as u64)).await;
        }
    }
}

pub const BITMAP: &[u8; 5] = &[0b11111, 0b11111, 0b11111, 0b11111, 0b11111];

static RIFF: &[Note] = &[
    Note(Pitch::E, 150),
    Note(Pitch::Silent, 150),
    Note(Pitch::G, 150),
    Note(Pitch::Silent, 150),
    Note(Pitch::A, 300),
    Note(Pitch::Silent, 150),
    Note(Pitch::E, 150),
    Note(Pitch::Silent, 150),
    Note(Pitch::G, 150),
    Note(Pitch::Silent, 150),
    Note(Pitch::AB, 150),
    Note(Pitch::Silent, 25),
    Note(Pitch::A, 300),
    Note(Pitch::Silent, 300),
    Note(Pitch::E, 150),
    Note(Pitch::Silent, 150),
    Note(Pitch::G, 150),
    Note(Pitch::Silent, 150),
    Note(Pitch::A, 300),
    Note(Pitch::Silent, 150),
    Note(Pitch::G, 150),
    Note(Pitch::Silent, 150),
    Note(Pitch::E, 300),
    Note(Pitch::Silent, 750),
];
