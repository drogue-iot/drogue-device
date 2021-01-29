//! Multi-actor package trait

use crate::prelude::{Actor, Address, Device, EventBus, Supervisor};

/// A package capable of configuring one or more actors collectively,
/// exposing a single actor's address as the ingress point.
///
/// A `Package` can be somewhat considered to be a sub-`Device` in
/// that it can provide a scoped method for mounting several sub-actors
/// and interrupts, bind them together, and inject the `EventBus`.
///
/// A single actor's `Address` shall be returned as the primary/initial
/// point of interaction with the package.
///
/// In some scenarios, a `Package` may consist of an `Actor` and an `Interrupt`
/// that work in tandem, while exporting the `Actor`'s address.
pub trait Package<D: Device, A: Actor> {
    /// Mount this package.
    fn mount(
        &'static self,
        bus_address: &Address<EventBus<D>>,
        supervisor: &mut Supervisor,
    ) -> Address<A>;
}
