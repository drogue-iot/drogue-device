use core::future::Future;
use drogue_device::*;

/// A package is a way to group actors working together.
///
/// The external actor dispatches work to the internal actor,
/// which performs the actual work.
pub struct MyPackage {
    a: ActorContext<'static, InternalActor>,
    b: ActorContext<'static, InternalActor>,
    external: ActorContext<'static, ExternalActor>,
}

impl MyPackage {
    pub fn new() -> Self {
        Self {
            a: ActorContext::new(InternalActor {
                name: "a",
                counter: 0,
            }),
            b: ActorContext::new(InternalActor {
                name: "b",
                counter: 0,
            }),
            external: ActorContext::new(ExternalActor),
        }
    }
}

/// The Package trait implementation to initialize this package
impl Package for MyPackage {
    /// The primary actor
    type Primary = ExternalActor;
    type Configuration = ();
    fn mount<S: ActorSpawner>(
        &'static self,
        _: Self::Configuration,
        spawner: S,
    ) -> Address<Self::Primary> {
        let a = self.a.mount((), spawner);
        let b = self.b.mount((), spawner);
        self.external.mount((a, b), spawner)
    }
}

#[derive(Clone, Copy)]
pub struct Increment;

/// The external actor dispatches work to the internal actors
pub struct ExternalActor;

impl Actor for ExternalActor {
    type Configuration = (
        Address<'static, InternalActor>,
        Address<'static, InternalActor>,
    );
    type Message<'m> = Increment;

    type OnMountFuture<'m, M>
    where
        M: 'm,
    = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        actors: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            log::info!("External started!");
            let (a, b) = actors;
            loop {
                match inbox.next().await {
                    Some(mut m) => match m.message() {
                        m => {
                            log::info!("Dispatching increment message");
                            a.notify(*m).unwrap();
                            b.notify(*m).unwrap();
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}

pub struct InternalActor {
    name: &'static str,
    counter: u32,
}

impl Actor for InternalActor {
    type Message<'m> = Increment;

    type OnMountFuture<'m, M>
    where
        M: 'm,
    = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            log::info!("[{}] started!", self.name);
            loop {
                match inbox.next().await {
                    Some(mut m) => match m.message() {
                        Increment => {
                            self.counter += 1;
                            log::info!("[{}]: {}", self.name, self.counter);
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}
