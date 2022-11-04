#![no_std]
#![feature(type_alias_impl_trait)]

mod boards;

use boards::*;
use drogue_device::*;

/// This trait defines the trait-based capabilities
/// required by a board and provides associated-types
/// in order to make referencing them easier with fewer
/// generics involved in the app itself.
pub trait BlinkyBoard {
    type Led: embedded_hal::digital::OutputPin;
    type Button: embedded_hal::digital::InputPin + embedded_hal_async::digital::Wait;

    fn new() -> (Self::Led, Self::Button);
}

#[main]
async fn main(_s: Spawner) {
    let (mut led, mut button) = Board::new();
    loop {
        button.wait_for_any_edge().await;
        if button.is_low() {
            led.set_high();
        } else {
            led.set_low();
        }
    }
}

/*
#[embassy_executor::spawnerentry]
async fn main<B>(led: B::Led, button: B::Button)
where
    B: BlinkyBoard,
{
    loop {}
}
*/
