use core::future::Future;
use drogue_device::{
    actors::button::{ButtonEvent, FromButtonEvent},
    Actor, Address, Inbox,
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
    type OnMountFuture<'a, M> where M: 'a  = impl Future<Output = ()> + 'a;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            loop {
                match *inbox.next().await.message() {
                    StatisticsCommand::PrintStatistics => {
                        defmt::info!("Character count: {}", self.character_counter)
                    }
                    StatisticsCommand::IncrementCharacterCount => self.character_counter += 1,
                }
            }
        }
    }
}
