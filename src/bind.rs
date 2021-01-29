//! Binding actors to other actors


use crate::actor::Actor;
use crate::address::Address;

/// Trait denoting an actor wishes to have another actor's address
/// bound into itself.
///
/// May be implemented several times per actor in order to bind multiple
/// dependencies.
pub trait Bind<A: Actor> {

    /// Perform the binding of the passed in address.
    fn on_bind(&'static mut self, address: Address<A>);
}
