#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use drogue_device::{
    bsp::boards::nrf52::microbit::*, traits::led::LedMatrix, Actor, ActorContext, Address, Board,
    Inbox,
};

use embassy_nrf::{pwm::*, Peripherals};

use core::future::Future;
use embassy::time::{Duration, Instant, Timer};
use futures::future::join;

use panic_probe as _;

static LED_MATRIX: ActorContext<LedMatrixActor> = ActorContext::new();
static DISCO: ActorContext<Disco> = ActorContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = Microbit::new(p);
    let matrix = LED_MATRIX.mount(spawner, LedMatrixActor::new(board.led_matrix, None));
    let disco = DISCO.mount(spawner, Disco::new(matrix));

    let pwm = SimplePwm::new_1ch(board.pwm0, board.p0_00);
    let mut speaker = PwmSpeaker::new(pwm);

    loop {
        for i in 0..RIFF.len() {
            join(disco.request(RIFF[i]).unwrap(), speaker.play_note(RIFF[i])).await;
        }
    }
}

pub struct Disco {
    display: Address<LedMatrixActor>,
}

impl Disco {
    pub fn new(display: Address<LedMatrixActor>) -> Self {
        Self { display }
    }
}

impl Actor for Disco {
    type Message<'m> = Note;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        M: 'm + Inbox<Self>,
        Self: 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            loop {
                if let Some(mut m) = inbox.next().await {
                    let note = *m.message();
                    if note.0 != Pitch::Silent {
                        let _ = self.display.apply(&BITMAP).await;
                        let _ = self.display.max_brightness();
                        let interval = Duration::from_millis(note.1 as u64 / 10);
                        let end = Instant::now() + Duration::from_millis(note.1 as u64);
                        while Instant::now() < end {
                            let _ = self.display.decrease_brightness();
                            Timer::after(interval).await;
                        }
                        let _ = self.display.clear().await;
                    } else {
                        Timer::after(Duration::from_millis(note.1 as u64)).await;
                    }
                }
            }
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
