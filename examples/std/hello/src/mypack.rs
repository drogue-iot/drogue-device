use super::myactor::*;
use core::sync::atomic::AtomicU32;
use drogue_device::*;

// A package is a way to wrap a package of actors and shared state together
// the actor in this package will use a different state than the others.
pub struct MyPack {
    counter: AtomicU32,
    c: ActorContext<'static, MyActor>,
}

impl MyPack {
    pub fn new() -> Self {
        Self {
            counter: AtomicU32::new(0),
            c: ActorContext::new(MyActor::new("c")),
        }
    }
}

// The Package trait by e implemented to initialize a package
impl Package for MyPack {
    type Primary = MyActor;
    type Configuration = ();
    fn mount(
        &'static self,
        _: Self::Configuration,
        spawner: &ActorSpawner,
    ) -> Address<Self::Primary> {
        self.c.mount(&self.counter, spawner)
    }
}
