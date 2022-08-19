#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use panic_probe as _;

use core::convert::TryFrom;
use drogue_device::{
    actors::button::{Button, ButtonEvent},
    bsp::boards::nrf52::microbit::*,
    Board,
};
use ector::{actor, spawn_actor, Actor, Address, Inbox};
use embassy_time::{Duration, Ticker, Timer};
use embassy_nrf::Peripherals;

use futures::{
    future::{select, Either},
    pin_mut, StreamExt,
};

/// A simple game where the led matrix is traversed at a fixed interval and you press the button
/// to light a red. You win when the whole board is lit.
struct Game {
    matrix: LedMatrix,
}

#[derive(Clone)]
pub enum GameMessage {
    Toggle,
}

impl TryFrom<ButtonEvent> for GameMessage {
    type Error = ();
    fn try_from(event: ButtonEvent) -> Result<Self, Self::Error> {
        match event {
            ButtonEvent::Released => Ok(GameMessage::Toggle),
            _ => Err(()),
        }
    }
}

impl Game {
    pub fn new(matrix: LedMatrix) -> Self {
        Self { matrix }
    }
}

#[actor]
impl Actor for Game {
    type Message<'m> = GameMessage;

    async fn on_mount<M>(&mut self, _: Address<GameMessage>, mut inbox: M)
    where
        M: Inbox<GameMessage> + 'm,
    {
        defmt::info!("Starting game! Press the 'A' button to lit the LED at the cursor.");
        let speed = Duration::from_millis(200);

        let mut coordinates: [[bool; 5]; 5] = [[false; 5]; 5];
        let mut cursor = 0;
        let (mut x, mut y) = (0, 0);
        let mut done = false;

        let mut render = Ticker::every(Duration::from_millis(5));
        while !done {
            self.matrix.on(x, y);
            // Race timeout and button press
            let timeout = Timer::after(speed);
            let event = inbox.next();
            pin_mut!(timeout);
            pin_mut!(event);

            let mut logic = select(timeout, event);
            loop {
                let tick = render.next();
                pin_mut!(tick);
                match select(tick, &mut logic).await {
                    Either::Left((_, _)) => {
                        self.matrix.render();
                    }
                    Either::Right((f, _)) => match f {
                        Either::Left(_) => {
                            break;
                        }
                        Either::Right(_) => {
                            // Set/unset
                            coordinates[y][x] = !coordinates[y][x];
                            break;
                        }
                    },
                }
            }

            // Unlit only if we're not set
            if !coordinates[y][x] {
                self.matrix.off(x, y)
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
            self.matrix.render();
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner, p: Peripherals) {
    // Using a board support package to simplify setup
    let board = Microbit::new(p);

    // An actor for the game logic
    let game = spawn_actor!(spawner, GAME, Game, Game::new(board.display));

    // Actor for button 'A'
    spawn_actor!(spawner, BUTTON_A, Button<PinButtonA, GameMessage>, Button::new(board.btn_a, game));
}
