use crate::handler::{Completion, Response};
use crate::prelude::{Actor, NotifyHandler, RequestHandler, ActorInfo};

use crate::alloc::HEAP;

pub struct Query;
pub struct Info {
    pub used: usize,
    pub free: usize,
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

impl Actor for Memory {}

impl RequestHandler<Query> for Memory {
    type Response = Info;

    fn on_request(self, message: Query) -> Response<Self, Self::Response> {
        Response::immediate(
            self,
            Info {
                used: unsafe { HEAP.as_ref().unwrap().used() },
                free: unsafe { HEAP.as_ref().unwrap().free() },
            },
        )
    }
}

impl NotifyHandler<Query> for Memory {
    fn on_notify(self, message: Query) -> Completion<Self> {
        let used = unsafe { HEAP.as_ref().unwrap().used() };
        let free = unsafe { HEAP.as_ref().unwrap().free() };
        log::info!("[{}] used={}, free={}", ActorInfo::name(), used, free);
        Completion::immediate(self)
    }
}
