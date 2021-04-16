use core::future::Future;
use core::pin::Pin;
use drogue_device::{
    actors::button::{ButtonEvent, FromButtonEvent},
    Actor,
};

pub struct Statistics {
    character_counter: u32,
}

impl Statistics {
    pub fn new() -> Self {
        Self {
            character_counter: 0,
        }
    }
}

pub enum StatisticsCommand {
    PrintStatistics,
    IncrementCharacterCount,
}

impl FromButtonEvent for StatisticsCommand {
    fn from(event: ButtonEvent) -> Option<StatisticsCommand> {
        match event {
            ButtonEvent::Released => Some(StatisticsCommand::PrintStatistics),
            ButtonEvent::Pressed => None,
        }
    }
}

impl Actor for Statistics {
    type Configuration = ();
    type Message<'a> = StatisticsCommand;
    type OnStartFuture<'a> = impl Future<Output = ()> + 'a;
    type OnMessageFuture<'a> = impl Future<Output = ()> + 'a;

    fn on_start(self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
        async move {}
    }

    fn on_message<'m>(
        mut self: Pin<&'m mut Self>,
        message: &'m mut Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            match *message {
                StatisticsCommand::PrintStatistics => {
                    defmt::info!("Character count: {}", self.character_counter)
                }
                StatisticsCommand::IncrementCharacterCount => self.character_counter += 1,
            }
        }
    }
}
