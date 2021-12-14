#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

use drogue_device::{
    actors::button::{Button, ButtonPressed},
    bsp::boards::nrf52::microbit::*,
    traits::led::LedMatrix as LedMatrixTrait,
    Actor, ActorContext, Address, Board, Inbox,
};

use embassy::time::{Duration, Timer};
use embassy_nrf::Peripherals;

use core::future::Future;
use futures::{
    future::{select, Either},
    pin_mut,
};

/// A simple game where the led matrix is traversed at a fixed interval and you press the button
/// to light a red. You win when the whole board is lit.
struct Game {
    matrix: Address<LedMatrixActor>,
}

#[derive(Clone)]
pub enum GameMessage {
    Toggle,
}

impl Game {
    pub fn new(matrix: Address<LedMatrixActor>) -> Self {
        Self { matrix }
    }
}

impl Actor for Game {
    type Message<'m> = GameMessage;
    type OnMountFuture<'m, M>
    where
        M: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            defmt::info!("Starting game! Press the 'A' button to lit the LED at the cursor.");
            let speed = Duration::from_millis(200);

            let mut coordinates: [[bool; 5]; 5] = [[false; 5]; 5];
            let mut cursor = 0;
            let (mut x, mut y) = (0, 0);
            let mut done = false;

            while !done {
                self.matrix.on(x, y).await.unwrap();
                // Race timeout and button press
                let timeout = Timer::after(speed);
                let event = inbox.next();
                pin_mut!(timeout);
                pin_mut!(event);
                match select(timeout, event).await {
                    // Timeout
                    Either::Left(_) => {}
                    // Set/unset
                    Either::Right(_) => {
                        coordinates[y][x] = !coordinates[y][x];
                    }
                }

                // Unlit only if we're not set
                if !coordinates[y][x] {
                    self.matrix.off(x, y).await.unwrap();
                }

                // Check if game is done
                done = true;
                for x in 0..5 {
                    for y in 0..5 {
                        if !coordinates[y][x] {
                            done = false;
                            break;
                        }
                    }
                }

                x = cursor % 5;
                y = (cursor / 5) % 5;
                cursor += 1;
            }
        }
    }
}

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    // Using a board support package to simplify setup
    let board = Microbit::new(p);

    // Led Matrix actor that will handle the display refresh loop and state of LED matrix
    static LED_MATRIX: ActorContext<LedMatrixActor> = ActorContext::new();

    // Mounting will start the display loop
    let matrix = LED_MATRIX.mount(spawner, LedMatrixActor::new(board.led_matrix, None));

    // An actor for the game logic
    static GAME: ActorContext<Game> = ActorContext::new();
    let game = GAME.mount(spawner, Game::new(matrix));

    // Actor for button 'A'
    static BUTTON_A: ActorContext<Button<ButtonA, ButtonPressed<Game>>> = ActorContext::new();
    BUTTON_A.mount(
        spawner,
        Button::new(board.button_a, ButtonPressed(game, GameMessage::Toggle)),
    );
}
