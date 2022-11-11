#![no_std]
#![no_main]
#![macro_use]
#![feature(type_alias_impl_trait)]
/// A simple game where the led matrix is traversed at a fixed interval and you press the button
/// to light a red. You win when the whole board is lit.
use defmt_rtt as _;
use panic_probe as _;

use {
    embassy_futures::select::{select, Either},
    embassy_sync::channel::{Channel, DynamicReceiver},
    embassy_time::{Duration, Ticker, Timer},
    futures::StreamExt,
    microbit_bsp::*,
};

type CS = embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    // Using a board support package to simplify setup
    let board = Microbit::default();

    // Channel for game events, buffering up to 10 events
    static EVENTS: Channel<CS, GameMessage, 10> = Channel::new();

    // Start the game logic
    spawner
        .spawn(run_game(board.display, EVENTS.receiver().into()))
        .unwrap();

    // Wait for button presses and submit game events
    let mut button = board.btn_a;
    loop {
        button.wait_for_any_edge().await;
        if button.is_low() {
            // Best effort delivery, to ensure the game is responsive
            let _ = EVENTS.try_send(GameMessage::Toggle);
        }
    }
}

/// A message for the game logic based on external input
#[derive(Clone)]
pub enum GameMessage {
    Toggle,
}

#[embassy_executor::task]
async fn run_game(mut matrix: LedMatrix, events: DynamicReceiver<'static, GameMessage>) {
    defmt::info!("Starting game! Press the 'A' button to lit the LED at the cursor.");
    let speed = Duration::from_millis(200);

    let mut coordinates: [[bool; 5]; 5] = [[false; 5]; 5];
    let mut cursor = 0;
    let (mut x, mut y) = (0, 0);
    let mut done = false;

    let mut render = Ticker::every(Duration::from_millis(5));
    while !done {
        matrix.on(x, y);
        // Race timeout and button press
        let timeout = Timer::after(speed);
        let event = events.recv();

        let mut logic = select(timeout, event);
        loop {
            let tick = render.next();
            match select(tick, &mut logic).await {
                Either::First(_) => {
                    matrix.render();
                }
                Either::Second(f) => match f {
                    Either::First(_) => {
                        break;
                    }
                    Either::Second(_) => {
                        // Set/unset
                        coordinates[y][x] = !coordinates[y][x];
                        break;
                    }
                },
            }
        }

        // Unlit only if we're not set
        if !coordinates[y][x] {
            matrix.off(x, y)
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
        matrix.render();
    }
}
