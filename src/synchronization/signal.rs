// Copyright The Embassy Project (https://github.com/akiles/embassy). Licensed under the Apache 2.0
// license

use core::cell::UnsafeCell;
use core::mem;
use core::task::{Context, Poll, Waker};

#[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64"))]
extern crate std;

pub struct Signal<T> {
    state: UnsafeCell<State<T>>,
    #[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64"))]
    lock: std::sync::Mutex<()>,
}

enum State<T> {
    None,
    Waiting(Waker),
    Signaled(T),
}

unsafe impl<T: Sized> Send for Signal<T> {}
unsafe impl<T: Sized> Sync for Signal<T> {}

impl<T: Sized> Signal<T> {
    pub fn new() -> Self {
        Self {
            state: UnsafeCell::new(State::None),
            #[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64"))]
            lock: std::sync::Mutex::new(()),
        }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64"))]
    fn critical_section<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        let guard = self.lock.lock().unwrap();
        f()
    }

    #[cfg(not(any(target_arch = "x86", target_arch = "x86_64", target_arch = "aarch64")))]
    fn critical_section<F, R>(&self, f: F) -> R
    where
        F: FnOnce() -> R,
    {
        crate::platform::with_critical_section(|_| f())
    }

    #[allow(clippy::single_match)]
    pub fn signal(&self, val: T) {
        self.critical_section(|| unsafe {
            let state = &mut *self.state.get();
            match mem::replace(state, State::Signaled(val)) {
                State::Waiting(waker) => waker.wake(),
                _ => {}
            }
        })
    }

    pub fn reset(&self) {
        self.critical_section(|| unsafe {
            let state = &mut *self.state.get();
            *state = State::None
        })
    }

    pub fn poll_wait(&self, cx: &mut Context<'_>) -> Poll<T> {
        self.critical_section(|| unsafe {
            let state = &mut *self.state.get();
            match state {
                State::None => {
                    *state = State::Waiting(cx.waker().clone());
                    Poll::Pending
                }
                State::Waiting(w) if w.will_wake(cx.waker()) => Poll::Pending,
                State::Waiting(_) => {
                    log::error!("waker overflow");
                    Poll::Pending
                }
                State::Signaled(_) => match mem::replace(state, State::None) {
                    State::Signaled(res) => Poll::Ready(res),
                    _ => Poll::Pending,
                },
            }
        })
    }

    pub fn signaled(&self) -> bool {
        self.critical_section(|| matches!(unsafe { &*self.state.get() }, State::Signaled(_)))
    }
}

impl<T: Sized> Default for Signal<T> {
    fn default() -> Self {
        Self::new()
    }
}
