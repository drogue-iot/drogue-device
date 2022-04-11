use core::convert::TryFrom;
use core::future::Future;
use drogue_device::{actors::button::ButtonEvent, Actor, Address, Inbox};

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

impl TryFrom<ButtonEvent> for StatisticsCommand {
    type Error = ();
    fn try_from(event: ButtonEvent) -> Result<StatisticsCommand, Self::Error> {
        match event {
            ButtonEvent::Released => Ok(StatisticsCommand::PrintStatistics),
            ButtonEvent::Pressed => Err(()),
        }
    }
}

impl Actor for Statistics {
    type Message<'a> = StatisticsCommand;

    type OnMountFuture<'a, M> = impl Future<Output = ()> + 'a
    where
        M: 'a + Inbox<Self::Message<'a>>;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self::Message<'m>>,
        mut inbox: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        async move {
            loop {
                match inbox.next().await {
                    StatisticsCommand::PrintStatistics => {
                        defmt::info!("Character count: {}", self.character_counter)
                    }
                    StatisticsCommand::IncrementCharacterCount => self.character_counter += 1,
                }
            }
        }
    }
}
