use crate::actor::Actor;
use crate::address::Address;
use crate::device::Device;

pub trait Bind<D: Device, A: Actor<D>> {
    fn on_bind(&'static mut self, address: Address<D, A>);
}
