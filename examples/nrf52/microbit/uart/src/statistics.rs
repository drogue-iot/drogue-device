use core::future::Future;
use core::pin::Pin;
use drogue_device::{
    actors::button::{ButtonEvent, FromButtonEvent},
    Actor, Inbox,
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

impl FromButtonEvent<StatisticsCommand> for Statistics {
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
    #[rustfmt::skip]
    type OnStartFuture<'a, M> where M: 'a  = impl Future<Output = ()> + 'a;

    fn on_start<'m, M>(&'m mut self, inbox: &'m mut M) -> Self::OnStartFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            loop {
                inbox
                    .process(|message| match message {
                        StatisticsCommand::PrintStatistics => {
                            defmt::info!("Character count: {}", self.character_counter)
                        }
                        StatisticsCommand::IncrementCharacterCount => self.character_counter += 1,
                    })
                    .await;
            }
        }
    }
}
