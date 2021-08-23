use core::future::Future;
use drogue_device::*;
use embassy::executor::{raw::Task, SpawnError};
use wasm_bindgen_futures::spawn_local;

#[derive(Clone, Copy)]
pub struct WasmSpawner;

impl WasmSpawner {
    pub fn new() -> Self {
        Self {}
    }
}

impl ActorSpawner for WasmSpawner {
    fn spawn<F: Future<Output = ()> + 'static>(
        &self,
        _: &'static Task<F>,
        f: F,
    ) -> Result<(), SpawnError> {
        spawn_local(f);
        Ok(())
    }
}

static LOCK: GlobalLock = GlobalLock::new();

/// Assume single-threadedness in WASM for now, so this global lock is really just a dummy
struct GlobalLock {}

impl GlobalLock {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn lock(&self) {}

    pub fn unlock(&self) {}
}

critical_section::custom_impl!(GlobalLock);

unsafe impl critical_section::Impl for GlobalLock {
    unsafe fn acquire() -> u8 {
        LOCK.lock();
        0
    }

    unsafe fn release(_token: u8) {
        LOCK.unlock();
    }
}
