use crate::prelude::*;

use crate::arena::{Arena, Info};
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
}

impl<A: Arena + 'static> RequestHandler<Query> for Memory<A> {
    type Response = Info;

    fn on_request(self, message: Query) -> Response<Self, Self::Response> {
        //let heap = unsafe { HEAP.as_ref().unwrap() };
        //Response::immediate(self, Info::new(unsafe { HEAP.as_ref().unwrap() }))
        Response::immediate(self, A::info())
    }
}

impl<A: Arena + 'static> NotifyHandler<Query> for Memory<A> {
    fn on_notify(self, message: Query) -> Completion<Self> {
        let info = A::info();
        log::info!(
            "[{}] used={}, free={} || high={}",
            ActorInfo::name(),
            info.used,
            info.free,
            info.high_watermark,
        );
        Completion::immediate(self)
    }
}
