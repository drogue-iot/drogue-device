use embassy::{
    blocking_mutex::raw::ThreadModeRawMutex,
    mutex::{Mutex, MutexGuard, TryLockError},
};

pub type SharedMutex = ThreadModeRawMutex;

pub struct Shared<T> {
    t: Mutex<SharedMutex, Option<T>>,
}

impl<T> Shared<T> {
    pub const fn new() -> Self {
        Self {
            t: Mutex::new(None),
        }
    }

    pub fn initialize<'a>(&'a self, t: T) -> Handle<'a, T> {
        if let Ok(mut guard) = self.t.try_lock() {
            guard.replace(t);
        }
        Handle { handle: &self.t }
    }
}

unsafe impl<T> Sync for Shared<T> {}

pub struct Handle<'a, T> {
    handle: &'a Mutex<SharedMutex, Option<T>>,
}

impl<'a, T> Clone for Handle<'a, T> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle,
        }
    }
}

pub struct HandleGuard<'a, T> {
    guard: MutexGuard<'a, SharedMutex, Option<T>>,
}

impl<'a, T> Handle<'a, T> {
    pub async fn lock(&self) -> HandleGuard<'_, T> {
        HandleGuard {
            guard: self.handle.lock().await,
        }
    }

    pub fn try_lock(&self) -> Result<HandleGuard<'_, T>, TryLockError> {
        let guard = self.handle.try_lock()?;
        Ok(HandleGuard { guard })
    }
}

use core::ops::{Deref, DerefMut};
impl<'a, T> Deref for HandleGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        self.guard.deref().as_ref().unwrap()
    }
}

impl<'a, T> DerefMut for HandleGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut().as_mut().unwrap()
    }
}
