use crate::prelude::*;

use crate::arena::Arena;
use crate::system::SystemArena;
use core::marker::PhantomData;
//use crate::arena::HEAP;

pub struct Query;

pub struct Memory<A: Arena = SystemArena> {
    arena: PhantomData<A>,
}

impl<A: Arena> Memory<A> {
    pub fn new() -> Self {
        Self { arena: PhantomData }
    }
}

impl<A: Arena> Default for Memory<A> {
    fn default() -> Self {
        Self::new()
    }
}

impl<A: Arena> Actor for Memory<A> {
    type Configuration = ();
    type Request = Query;
    type Response = ();
    type DeferredFuture = DefaultDeferred<Self>;
    type ImmediateFuture = DefaultImmediate<Self>;
    fn on_request(self, message: Query) -> Response<Self> {
        let info = A::info();
        log::info!(
            "[{}] used={}, free={} || high={}",
            ActorInfo::name(),
            info.used,
            info.free,
            info.high_watermark,
        );
        Response::immediate(self, ())
    }
}
