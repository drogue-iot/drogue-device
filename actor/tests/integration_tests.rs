#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "std")]
mod tests {
    use core::future::Future;
    use drogue_actor::*;
    use embassy::executor::Spawner;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::{sync::mpsc, thread, time::Duration};

    static INITIALIZED: AtomicU32 = AtomicU32::new(0);

    #[test]
    fn test_device_setup() {
        pub struct MyActor {
            value: &'static AtomicU32,
        }

        pub struct Add(u32);
        impl Actor for MyActor {
            type Message<'m> = Add;

            type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm where M: 'm + Inbox<Self>;

            fn on_mount<'m, M>(
                &'m mut self,
                _: Address<Self>,
                inbox: &'m mut M,
            ) -> Self::OnMountFuture<'m, M>
            where
                M: Inbox<Self> + 'm,
            {
                async move {
                    loop {
                        let mut message = inbox.next().await.unwrap();
                        self.value.fetch_add(message.message().0, Ordering::SeqCst);
                    }
                }
            }
        }

        #[embassy::main]
        async fn main(spawner: Spawner) {
            static ACTOR: ActorContext<MyActor> = ActorContext::new();

            let a_addr = ACTOR.mount(
                spawner,
                MyActor {
                    value: &INITIALIZED,
                },
            );

            let _ = a_addr.notify(Add(10));
        }

        std::thread::spawn(move || {
            main();
        });

        panic_after(Duration::from_secs(10), move || {
            while INITIALIZED.load(Ordering::SeqCst) != 10 {
                std::thread::sleep(Duration::from_secs(1))
            }
        })
    }

    fn panic_after<T, F>(d: Duration, f: F) -> T
    where
        T: Send + 'static,
        F: FnOnce() -> T,
        F: Send + 'static,
    {
        let (done_tx, done_rx) = mpsc::channel();
        let handle = thread::spawn(move || {
            let val = f();
            done_tx.send(()).expect("Unable to send completion signal");
            val
        });

        match done_rx.recv_timeout(d) {
            Ok(_) => handle.join().expect("Thread panicked"),
            Err(_) => panic!("Thread took too long"),
        }
    }
}
