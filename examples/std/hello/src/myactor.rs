use core::future::Future;
use core::sync::atomic::{AtomicU32, Ordering};
use drogue_device::*;

pub struct MyActor {
    name: &'static str,
    counter: Option<&'static AtomicU32>,
}

impl MyActor {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            counter: None,
        }
    }
}

impl Actor for MyActor {
    type Configuration = &'static AtomicU32;
    type Message<'a> = SayHello<'a>;

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        self.counter.replace(config);
        async move {
            log::info!("[{}] started!", self.name);
            loop {
                match inbox.next().await {
                    Some((m, r)) => r.respond({
                        let count = self.counter.unwrap().fetch_add(1, Ordering::SeqCst);
                        log::info!("[{}] hello {}: {}", self.name, m.0, count);
                    }),
                    _ => {}
                }
            }
        }
    }
}

pub struct SayHello<'m>(pub &'m str);
