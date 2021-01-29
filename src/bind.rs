use crate::actor::Actor;
use crate::address::Address;
use crate::device::Device;

pub trait Bind<A: Actor> {
    fn on_bind(&'static mut self, address: Address<A>);
}
