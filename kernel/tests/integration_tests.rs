
use core::fmt::Debug;
use core::task::{Context, Poll};
use std::sync::Once;
use drogue_device::prelude::*;
use futures::executor::block_on;
use heapless::{consts, ArrayLength};
use std::time::{Duration, SystemTime};

static INIT: Once = Once::new();

fn setup() {
    INIT.call_once(|| {
        env_logger::init();
    });
}

// Completely and totally not safe.
fn staticize<A: Actor, Q: ArrayLength<SignalSlot> + ArrayLength<ActorMessage<A>>>(
    runner: &ActorContext<A, Q>,
) -> &'static ActorContext<A, Q> {
    unsafe { core::mem::transmute::<_, &'static ActorContext<A, Q>>(runner) }
}

#[test]
fn launch_actors() {
    setup();
    log::info!("Creating executor");
    let mut executor = ActorExecutor::new();
    log::info!("Creating actors ");
    let foo_runner = staticize(&ActorContext::<_, consts::U4>::new(MyActor::new("foo")));
    let bar_runner = staticize(&ActorContext::<_, consts::U4>::new(MyActor::new("bar")));

    let foo_addr = Address::new(foo_runner);
    let bar_addr = Address::new(bar_runner);

    log::info!("mounting...");
    foo_runner.mount(bar_addr, &mut executor);
    bar_runner.mount(foo_addr, &mut executor);

    log::info!("spawning executor");
    std::thread::spawn(move || {
        executor.run_forever();
    });

    let mut foo_req = MyMessage::new(1, 2, 2);
    let mut bar_req = MyMessage::new(3, 4, 2);

    let foo_fut = foo_addr.process(&mut foo_req);
    let bar_fut = bar_addr.process(&mut bar_req);

    log::info!("block on foo");
    block_on(foo_fut);
    log::info!("complete foo");
    log::info!("block on bar");
    block_on(bar_fut);
    log::info!("complete bar");
    // Cheat and use other executor for the test
    log::info!("Foo result: {:?}", foo_req);
    log::info!("Bar result: {:?}", bar_req);
}

struct MyActor<'c> {
    name: &'static str,
    other: Option<Address<'c, MyActor<'c>>>,
}

impl<'a> MyActor<'a> {
    pub fn new(name: &'static str) -> Self {
        Self { name, other: None }
    }
}

#[derive(Debug)]
pub struct MyMessage {
    a: u8,
    b: u8,
    delay: u8,
    started_at: Option<SystemTime>,
    c: Option<u8>,
}

impl MyMessage {
    pub fn new(a: u8, b: u8, delay: u8) -> Self {
        Self {
            a,
            b,
            delay,
            started_at: None,
            c: None,
        }
    }
}

impl<'c> Actor for MyActor<'c> {
    type Message = MyMessage;
    type Configuration = Address<'c, MyActor<'c>>;

    fn mount(&mut self, config: Self::Configuration) {
        self.other.replace(config);
        log::info!("[{}] mounted!", self.name);
    }

    fn poll_message(&mut self, message: &mut Self::Message, cx: &mut Context<'_>) -> Poll<()> {
        match message.started_at {
            None => {
                log::info!("[{}] delaying request: {:?}", self.name, message);
                message.started_at.replace(SystemTime::now());
                let waker = cx.waker().clone();
                let delay = message.delay;
                let name = self.name;
                std::thread::spawn(move || {
                    log::info!("[{}] sleeping for {}", name, delay);
                    std::thread::sleep(Duration::from_secs(delay as u64));
                    log::info!("[{}] waking for {}", name, delay);
                    waker.wake();
                });
                Poll::Pending
            }
            Some(time) => {
                if let Ok(elapsed) = time.elapsed() {
                    log::info!("[{}] woken after {:?}", self.name, elapsed.as_secs());
                    if elapsed.as_secs() >= message.delay as u64 {
                        log::info!("[{}] completed request: {:?}", self.name, message);
                        return Poll::Ready(());
                    }
                }
                log::info!("[{}] still pending", self.name);
                Poll::Pending
            }
        }
    }
}
