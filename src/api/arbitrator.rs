use crate::prelude::*;
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};
use heapless::{consts::*, spsc::Queue};

pub struct Shared {
    available: AtomicBool,
    waiters: RefCell<Queue<Waker, U8>>,
}

impl Shared {
    fn new() -> Self {
        Self {
            available: AtomicBool::new(false),
            waiters: RefCell::new(Queue::new()),
        }
    }

    fn begin_transaction(&self) -> bool {
        self.available
            .compare_and_swap(true, false, Ordering::AcqRel)
    }

    fn end_transaction(&self) {
        self.available.store(true, Ordering::Release);
        if let Some(next) = self.waiters.borrow_mut().dequeue() {
            next.wake()
        }
    }

    fn add_waiter(&self, waker: Waker) {
        self.waiters.borrow_mut().enqueue(waker);
    }
}

pub struct Arbitrator<BUS>
where
    BUS: Actor + 'static,
{
    shared: Shared,
    arbitrator: ActorContext<BusArbitrator<BUS>>,
}

impl<BUS> Arbitrator<BUS>
where
    BUS: Actor + 'static,
{
    pub fn new() -> Self {
        Self {
            shared: Shared::new(),
            arbitrator: ActorContext::new(BusArbitrator::new()),
        }
    }
}

impl<BUS> Package for Arbitrator<BUS>
where
    BUS: Actor + 'static,
{
    type Primary = BusArbitrator<BUS>;
    type Configuration = Address<BUS>;

    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        let addr = self.arbitrator.mount((&self.shared, config), supervisor);
        self.shared.available.store(true, Ordering::Release);
        addr
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.arbitrator.address()
    }
}

pub struct BusArbitrator<BUS>
where
    BUS: Actor + 'static,
{
    shared: Option<&'static Shared>,
    address: Option<Address<Self>>,
    bus: Option<Address<BUS>>,
}

impl<BUS> Actor for BusArbitrator<BUS>
where
    BUS: Actor + 'static,
{
    type Configuration = (&'static Shared, Address<BUS>);

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.address.replace(address);
        self.shared.replace(config.0);
        self.bus.replace(config.1);
    }
}

impl<BUS> BusArbitrator<BUS>
where
    BUS: Actor + 'static,
{
    pub fn new() -> Self {
        Self {
            shared: None,
            address: None,
            bus: None,
        }
    }
}

struct BeginTransaction;
struct EndTransaction;

impl<BUS> RequestHandler<BeginTransaction> for BusArbitrator<BUS>
where
    BUS: Actor + 'static,
{
    type Response = BusTransaction<BUS>;

    fn on_request(self, message: BeginTransaction) -> Response<Self, Self::Response> {
        let future = BeginTransactionFuture::new(
            &self.shared.unwrap(),
            self.address.unwrap(),
            self.bus.unwrap(),
        );
        Response::immediate_future(self, future)
    }
}

impl<BUS> NotifyHandler<EndTransaction> for BusArbitrator<BUS>
where
    BUS: Actor + 'static,
{
    fn on_notify(mut self, message: EndTransaction) -> Completion<Self> {
        self.shared.unwrap().end_transaction();
        Completion::immediate(self)
    }
}

impl<BUS> Address<BusArbitrator<BUS>>
where
    BUS: Actor + 'static,
{
    pub async fn begin_transaction(&self) -> BusTransaction<BUS> {
        self.request(BeginTransaction).await
    }
}

pub struct BusTransaction<BUS>
where
    BUS: Actor + 'static,
{
    arbitrator: Address<BusArbitrator<BUS>>,
    pub(crate) bus: Address<BUS>,
}

impl<BUS> BusTransaction<BUS>
where
    BUS: Actor + 'static,
{
    fn new(arbitrator: Address<BusArbitrator<BUS>>, bus: Address<BUS>) -> Self {
        Self { arbitrator, bus }
    }
}

impl<BUS> Drop for BusTransaction<BUS>
where
    BUS: Actor + 'static,
{
    fn drop(&mut self) {
        self.arbitrator.notify(EndTransaction {});
    }
}

pub struct BeginTransactionFuture<BUS>
where
    BUS: Actor + 'static,
{
    shared: &'static Shared,
    arbitrator: Address<BusArbitrator<BUS>>,
    bus: Address<BUS>,
    waiting: bool,
}

impl<BUS> BeginTransactionFuture<BUS>
where
    BUS: Actor + 'static,
{
    fn new(
        shared: &'static Shared,
        arbitrator: Address<BusArbitrator<BUS>>,
        bus: Address<BUS>,
    ) -> Self {
        Self {
            shared,
            arbitrator,
            bus,
            waiting: false,
        }
    }
}
impl<BUS> Future for BeginTransactionFuture<BUS>
where
    BUS: Actor + 'static,
{
    type Output = BusTransaction<BUS>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.shared.begin_transaction() {
            Poll::Ready(BusTransaction::new(self.arbitrator, self.bus))
        } else {
            if !self.waiting {
                self.shared.add_waiter(cx.waker().clone());
                self.waiting = true;
            }
            Poll::Pending
        }
    }
}
