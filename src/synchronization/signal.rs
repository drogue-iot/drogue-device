// Copyright The Embassy Project (https://github.com/akiles/embassy). Licensed under the Apache 2.0
// license

use core::cell::UnsafeCell;
use core::mem;
use core::task::{Context, Poll, Waker};

pub struct Signal<T> {
    state: UnsafeCell<State<T>>,
}

enum State<T> {
    None,
    Waiting(Waker),
    Signaled(T),
}

unsafe impl<T: Sized> Send for Signal<T> {}
unsafe impl<T: Sized> Sync for Signal<T> {}

impl<T: Sized> Signal<T> {
    pub const fn new() -> Self {
        Self {
            state: UnsafeCell::new(State::None),
        }
    }

    pub fn signal(&self, val: T) {
        cortex_m::interrupt::free(|_| unsafe {
            let state = &mut *self.state.get();
            match mem::replace(state, State::Signaled(val)) {
                State::Waiting(waker) => waker.wake(),
                _ => {}
            }
        })
    }

    pub fn reset(&self) {
        cortex_m::interrupt::free(|_| unsafe {
            let state = &mut *self.state.get();
            *state = State::None
        })
    }

    pub fn poll_wait(&self, cx: &mut Context<'_>) -> Poll<T> {
        cortex_m::interrupt::free(|_| unsafe {
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
        cortex_m::interrupt::free(|_| matches!(unsafe { &*self.state.get() }, State::Signaled(_)))
    }
}
