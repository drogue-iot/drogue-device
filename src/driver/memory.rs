use crate::handler::{Completion, Response};
use crate::prelude::{Actor, ActorInfo, NotifyHandler, RequestHandler};

use crate::alloc::cortex_m::CortexMHeap;
use crate::alloc::HEAP;

pub struct Query;
pub struct Info {
    pub used: usize,
    pub free: usize,
    pub high_watermark: usize,
}

impl Info {
    fn new(heap: &CortexMHeap) -> Self {
        Self {
            used: heap.used(),
            free: heap.free(),
            high_watermark: heap.high_watermark(),
        }
    }
}

pub struct Memory {}

impl Memory {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}

impl Actor for Memory {
    type Configuration = ();
}

impl RequestHandler<Query> for Memory {
    type Response = Info;

    fn on_request(self, message: Query) -> Response<Self, Self::Response> {
        let heap = unsafe { HEAP.as_ref().unwrap() };
        Response::immediate(self, Info::new(unsafe { HEAP.as_ref().unwrap() }))
    }
}

impl NotifyHandler<Query> for Memory {
    fn on_notify(self, message: Query) -> Completion<Self> {
        let info = Info::new(unsafe { HEAP.as_ref().unwrap() });
        log::info!(
            "[{}] used={}, free={} high={}",
            ActorInfo::name(),
            info.used,
            info.free,
            info.high_watermark,
        );
        Completion::immediate(self)
    }
}
