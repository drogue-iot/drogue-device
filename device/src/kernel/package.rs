use super::actor::{Actor, ActorSpawner, Address};

pub trait Package {
    type Primary: Actor;
    type Configuration = ();
    fn mount(
        &'static self,
        config: Self::Configuration,
        spawner: &ActorSpawner,
    ) -> Address<Self::Primary>;
}
