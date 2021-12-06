#![no_std]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use core::future::Future;
use core::marker::PhantomData;
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
    _marker: PhantomData<B>,
}

impl<B: BlinkyBoard> Default for BlinkyApp<B> {
    fn default() -> Self {
        Self {
            _marker: Default::default(),
        }
    }
}

/// Implementation of the `App` template methods for code-organization.
impl<B: BlinkyBoard> BlinkyDevice<B> {
    /// The `Device` is exactly the typical drogue-device Device.
    pub fn new(components: BlinkyConfiguration<B>) -> Self {
        BlinkyDevice {
            app: ActorContext::new(Default::default()),
            led: ActorContext::new(actors::led::Led::new(components.led)),
            button: ActorContext::new(actors::button::Button::new(components.control_button)),
        }
    }

    /// This is exactly the same operation performed during normal mount cycles
    /// in a non-BSP example.
    pub async fn mount(&'static self, spawner: Spawner) {
        let led = self.led.mount((), spawner);
        let app = self.app.mount(led, spawner);
        self.button.mount(app.into(), spawner);
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
    type Configuration = Address<'static, actors::led::Led<B::Led>>;

    type Message<'m> = Command;

    type OnMountFuture<'m, M>
    where
        M: 'm,
    = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        let led = config;
        async move {
            loop {
                match inbox.next().await {
                    Some(mut msg) => match msg.message() {
                        Command::TurnOn => {
                            defmt::info!("got inbox ON");
                            led.notify(LedMessage::On).ok();
                        }
                        Command::TurnOff => {
                            defmt::info!("got inbox OFF");
                            led.notify(LedMessage::Off).ok();
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
    app: ActorContext<'static, BlinkyApp<B>>,
    led: ActorContext<'static, actors::led::Led<B::Led>>,
    button: ActorContext<
        'static,
        actors::button::Button<B::ControlButton, ButtonEventDispatcher<BlinkyApp<B>>>,
    >,
}
