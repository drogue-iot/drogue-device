#![no_std]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::actors;
use drogue_device::actors::led::LedMessage;
use drogue_device::traits;
use ector::ActorContext;
use embassy_executor::executor::Spawner;

/// This trait defines the trait-based capabilities
/// required by a board and provides associated-types
/// in order to make referencing them easier with fewer
/// generics involved in the app itself.
pub trait BlinkyBoard
where
    Self: 'static,
{
    type Led: traits::led::Led;
    type ControlButton: traits::button::Button;
}

/// These are the trait-based components required by the app.
/// Members must be public so they can be slurped off.
/// Types should reference the associated types defined
/// by the board trait above.
pub struct BlinkyConfiguration<B: BlinkyBoard> {
    pub led: B::Led,
    pub control_button: B::ControlButton,
}

/// Implementation of the `App` template methods for code-organization.
impl<B: BlinkyBoard> BlinkyDevice<B> {
    /// The `Device` is exactly the typical drogue-device Device.
    pub fn new() -> Self {
        BlinkyDevice {
            led: ActorContext::new(),
            button: ActorContext::new(),
        }
    }

    /// This is exactly the same operation performed during normal mount cycles
    /// in a non-BSP example.
    pub async fn mount(&'static self, spawner: Spawner, components: BlinkyConfiguration<B>) {
        let led_address = self
            .led
            .mount(spawner, actors::led::Led::new(components.led));
        self.button.mount(
            spawner,
            actors::button::Button::new(components.control_button, led_address),
        );
    }
}

/// The ultimate drogue-device Device, per usual.
/// Defined using the type-aliases for the app-specific board.
pub struct BlinkyDevice<B: BlinkyBoard + 'static> {
    led: ActorContext<actors::led::Led<B::Led>>,
    button: ActorContext<actors::button::Button<B::ControlButton, LedMessage>>,
}
