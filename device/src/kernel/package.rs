use super::actor::{Actor, ActorContext, ActorSpawner, Address};
use super::signal::SignalSlot;

/// The package trait provides a way to bundle one or more actors and
/// additional state in a package that can be used by other components.
///
/// A Package is mounted with its desired configuration, and has a primary
/// Actor that it provides the Address of when mounted.
pub trait Package {
    /// The primary Actor for this package.
    type Primary: Actor;

    /// The expected configuration when mounting.
    type Configuration = ();

    /// Mount this package, providing the configuration and a reference
    /// to a spawner used when mounting internal actors of the Package.
    fn mount<S: ActorSpawner>(
        &'static self,
        config: Self::Configuration,
        spawner: S,
    ) -> Address<Self::Primary>;
}

impl<A: Actor + 'static, const QUEUE_SIZE: usize> Package for ActorContext<'static, A, QUEUE_SIZE>
where
    [SignalSlot<<A as Actor>::Response>; QUEUE_SIZE]: Default,
{
    type Primary = A;
    type Configuration = A::Configuration;
    fn mount<S: ActorSpawner>(
        &'static self,
        config: Self::Configuration,
        spawner: S,
    ) -> Address<Self::Primary> {
        self.mount(config, spawner)
    }
}
