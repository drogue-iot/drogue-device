use core::future::Future;
use drogue_device::*;

/// A package is a way to group actors working together.
///
/// The external actor dispatches work to the internal actor,
/// which performs the actual work.
pub struct MyPackage {
    a: ActorContext<InternalActor>,
    b: ActorContext<InternalActor>,
    external: ActorContext<ExternalActor>,
}

impl MyPackage {
    pub fn new() -> Self {
        Self {
            a: ActorContext::new(),

            b: ActorContext::new(),
            external: ActorContext::new(),
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
        let a = self.a.mount(
            spawner,
            InternalActor {
                name: "a",
                counter: 0,
            },
        );
        let b = self.b.mount(
            spawner,
            InternalActor {
                name: "b",
                counter: 0,
            },
        );
        self.external.mount(spawner, ExternalActor(a, b))
    }
}

#[derive(Clone, Copy)]
pub struct Increment;

/// The external actor dispatches work to the internal actors
pub struct ExternalActor(Address<InternalActor>, Address<InternalActor>);

impl Actor for ExternalActor {
    type Message<'m> = Increment;

    type OnMountFuture<'m, M>
    where
        M: 'm,
    = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            log::info!("External started!");
            let (a, b) = (self.0, self.1);
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
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
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
