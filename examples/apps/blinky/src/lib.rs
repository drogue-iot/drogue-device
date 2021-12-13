#![no_std]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use core::future::Future;
use drogue_device::actors::button::{ButtonEvent, ButtonEventDispatcher, FromButtonEvent};
use drogue_device::actors::led::LedMessage;
use drogue_device::traits;
use drogue_device::ActorContext;
use drogue_device::{actors, Actor, Address, Inbox};
use embassy::executor::Spawner;

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
// by the board trait above.
pub struct BlinkyConfiguration<B: BlinkyBoard> {
    pub led: B::Led,
    pub control_button: B::ControlButton,
}

/// The actual application structure. There is no
/// requirement that this application have any data
/// or be an `Actor` implementation itself. It just
/// so happens to be one in this example.
pub struct BlinkyApp<B: BlinkyBoard + 'static> {
    led: Address<actors::led::Led<B::Led>>,
}

impl<B: BlinkyBoard> BlinkyApp<B> {
    fn new(led: Address<actors::led::Led<B::Led>>) -> Self {
        Self { led }
    }
}

/// Implementation of the `App` template methods for code-organization.
impl<B: BlinkyBoard> BlinkyDevice<B> {
    /// The `Device` is exactly the typical drogue-device Device.
    pub fn new() -> Self {
        BlinkyDevice {
            app: ActorContext::new(),
            led: ActorContext::new(),
            button: ActorContext::new(),
        }
    }

    /// This is exactly the same operation performed during normal mount cycles
    /// in a non-BSP example.
    pub async fn mount(&'static self, spawner: Spawner, components: BlinkyConfiguration<B>) {
        let led = self
            .led
            .mount(spawner, actors::led::Led::new(components.led));
        let app = self.app.mount(spawner, BlinkyApp::new(led));
        self.button.mount(
            spawner,
            actors::button::Button::new(components.control_button, app.into()),
        );
    }
}

/// App-specific commands for the App actor.
pub enum Command {
    TurnOn,
    TurnOff,
}

/// Typical Actor implementation for an app object.
/// Dispatches its `Command` messages to turn the
/// LED on or off.
///
/// These commands are ultimately triggered by a Button actor
/// wrapping the `Button`-traited component.
impl<B: BlinkyBoard> Actor for BlinkyApp<B> {
    type Message<'m> = Command;

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
            loop {
                match inbox.next().await {
                    Some(mut msg) => match msg.message() {
                        Command::TurnOn => {
                            defmt::info!("got inbox ON");
                            self.led.notify(LedMessage::On).ok();
                        }
                        Command::TurnOff => {
                            defmt::info!("got inbox OFF");
                            self.led.notify(LedMessage::Off).ok();
                        }
                    },
                    None => {
                        defmt::info!("got inbox NONE");
                    }
                }
            }
        }
    }
}

/// ButtonEvent to App command translator.
impl<B: BlinkyBoard> FromButtonEvent<Command> for BlinkyApp<B> {
    fn from(event: ButtonEvent) -> Option<Command>
    where
        Self: Sized,
    {
        match event {
            ButtonEvent::Pressed => Some(Command::TurnOn),
            ButtonEvent::Released => Some(Command::TurnOff),
        }
    }
}

/// The ultimate drogue-device Device, per usual.
/// Defined using the type-aliases for the app-specific board.
pub struct BlinkyDevice<B: BlinkyBoard + 'static> {
    app: ActorContext<BlinkyApp<B>>,
    led: ActorContext<actors::led::Led<B::Led>>,
    button:
        ActorContext<actors::button::Button<B::ControlButton, ButtonEventDispatcher<BlinkyApp<B>>>>,
}
