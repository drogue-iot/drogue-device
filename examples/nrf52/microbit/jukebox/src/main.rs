#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use drogue_device::{
    actors::led::matrix::LedMatrixActor, bsp::boards::nrf52::microbit::*, ActorContext, Board,
};

use embassy_nrf::{
    gpio::{AnyPin, NoPin, Output},
    pwm::*,
    Peripherals,
};

use panic_probe as _;

mod speaker;
use speaker::*;

static LED_MATRIX: ActorContext<LedMatrixActor<Output<'static, AnyPin>, 5, 5>> =
    ActorContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = Microbit::new(p);
    let matrix = LED_MATRIX.mount(spawner, LedMatrixActor::new(board.led_matrix, None));
    let pwm = SimplePwm::new(board.pwm0, board.p0_00, NoPin, NoPin, NoPin);
    let mut speaker = PwmSpeaker::new(pwm, matrix);

    loop {
        for i in 0..RIFF.len() {
            speaker.play_sample(&RIFF[i]).await;
        }
    }
}

static RIFF: &[Sample<'static>] = &[Sample::new(&[
    Note(Pitch::E as u32, 150),
    Note(0, 150),
    Note(Pitch::G as u32, 150),
    Note(0, 150),
    Note(Pitch::A as u32, 300),
    Note(0, 150),
    Note(Pitch::E as u32, 150),
    Note(0, 150),
    Note(Pitch::G as u32, 150),
    Note(0, 150),
    Note(Pitch::AB as u32, 150),
    Note(0, 25),
    Note(Pitch::A as u32, 300),
    Note(0, 300),
    Note(Pitch::E as u32, 150),
    Note(0, 150),
    Note(Pitch::G as u32, 150),
    Note(0, 150),
    Note(Pitch::A as u32, 300),
    Note(0, 150),
    Note(Pitch::G as u32, 150),
    Note(0, 150),
    Note(Pitch::E as u32, 300),
    Note(0, 750),
])];
