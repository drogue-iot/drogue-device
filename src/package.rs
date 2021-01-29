use crate::prelude::{Address, EventBus, Supervisor, Device, Actor};

pub trait Package<D: Device, A: Actor> {
    fn mount(&'static self, bus_address: &Address<EventBus<D>>, supervisor: &mut Supervisor) -> Address<A>;
}