use crate::actor::Actor;
use crate::address::Address;

pub trait Bind<A: Actor> {
    fn on_bind(&'static mut self, address: Address<A>);
}
