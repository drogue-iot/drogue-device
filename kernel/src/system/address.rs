use crate::system::actor::{Actor, ActorHandle};

pub struct Address<'a, A: Actor> {
    runner: &'a dyn ActorHandle<A>,
}

impl<A: Actor> Copy for Address<'_, A> {}

impl<A: Actor> Clone for Address<'_, A> {
    fn clone(&self) -> Self {
        Self {
            runner: self.runner,
        }
    }
}

impl<'a, A: Actor> Address<'a, A> {
    pub fn new(runner: &'a dyn ActorHandle<A>) -> Self {
        Self { runner }
    }

    pub async fn process(&self, message: &mut A::Message) {
        self.runner.process_message(message).await
    }
}
