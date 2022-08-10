use embassy_util::Forever;
use futures_intrusive::sync::{LocalMutex, LocalMutexGuard};

pub struct Shared<T> {
    t: Forever<LocalMutex<T>>,
}

impl<T> Shared<T> {
    pub const fn new() -> Self {
        Self { t: Forever::new() }
    }

    pub fn initialize<'a>(&'static self, t: T) -> Handle<'a, T> {
        let handle = self.t.put(LocalMutex::new(t, true));
        Handle { handle }
    }
}

unsafe impl<T> Sync for Shared<T> {}

pub struct Handle<'a, T>
where
    T: 'a,
{
    handle: &'a LocalMutex<T>,
}

impl<'a, T> Clone for Handle<'a, T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
        }
    }
}

pub struct HandleGuard<'a, T> {
    guard: LocalMutexGuard<'a, T>,
}

impl<'a, T> Handle<'a, T> {
    pub async fn lock(&self) -> HandleGuard<'_, T> {
        HandleGuard {
            guard: self.handle.lock().await,
        }
    }

    pub fn try_lock(&self) -> Option<HandleGuard<'_, T>> {
        self.handle.try_lock().map(|guard| HandleGuard { guard })
    }
}

use core::ops::{Deref, DerefMut};
impl<'a, T> Deref for HandleGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}

impl<'a, T> DerefMut for HandleGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut()
    }
}
