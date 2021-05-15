use core::future::Future;
use core::pin::Pin;
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
    type OnStartFuture<'a> = impl Future<Output = ()> + 'a;
    type OnMessageFuture<'a> = impl Future<Output = ()> + 'a;

    fn on_mount(&mut self, config: Self::Configuration) {
        self.counter.replace(config);
    }

    fn on_start(self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
        async move { log::info!("[{}] started!", self.name) }
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            let count = self.counter.unwrap().fetch_add(1, Ordering::SeqCst);
            log::info!("[{}] hello {}: {}", self.name, message.0, count);
        }
    }
}

pub struct SayHello<'m>(pub &'m str);
