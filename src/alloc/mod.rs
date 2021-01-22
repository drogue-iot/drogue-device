use core::mem::{size_of, align_of};
use core::cell::UnsafeCell;
use core::ops::Deref;
use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};
use core::ops::DerefMut;
use crate::alloc::cortex_m::CortexMHeap;
use core::fmt::{Debug, Formatter};
use core::ptr::drop_in_place;

pub mod cortex_m;

pub static mut HEAP: Option<CortexMHeap> = None;

#[doc(hidden)]
#[macro_export]
macro_rules! init_heap {
    ($size:literal) => {
        static mut HEAP_MEMORY: [u8; $size] = [0; $size];
        unsafe {
            $crate::alloc::HEAP.replace( $crate::alloc::cortex_m::CortexMHeap::new( &HEAP_MEMORY ));
        }
    }
}

pub fn alloc<T>(val: T) -> Option<&'static mut T>
{
    unsafe {
        HEAP.as_mut().unwrap().alloc_init(val)
    }
}

#[repr(transparent)]
pub struct Box<T: ?Sized> {
    pub(crate) pointer: UnsafeCell<* mut T>,
}

impl<T: ?Sized> Box<T> {
    pub fn new(val: &'static mut T) -> Self {
        Self {
            pointer: UnsafeCell::new(val)
        }
    }
}

impl<T: ?Sized> Deref for Box<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &**self.pointer.get()
        }
    }
}

impl<T: ?Sized> DerefMut for Box<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            &mut **self.pointer.get()
        }
    }
}

impl<T: Future + ?Sized + 'static> Future for Box<T> {
    type Output = T::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            T::poll(Pin::new_unchecked(&mut *self), cx)
        }
    }
}

impl<T: ?Sized> Drop for Box<T> {
    fn drop(&mut self) {
        unsafe {
            drop_in_place( self.pointer.get() );
            HEAP.as_ref().unwrap().dealloc_object(
                *self.pointer.get() as *mut u8,
            );
        }
    }
}

impl<T: ?Sized + Debug> Debug for Box<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        unsafe {
            (&(*self.pointer.get() as * const T)).fmt(f)
        }
    }
}

struct RcBox<T: ?Sized> {
    count: u8,
    value: T,
}

impl<T> RcBox<T> {
    pub fn new(value: T) -> Self {
        Self {
            count: 0,
            value,
        }
    }
}

pub struct Rc<T> {
    pointer: UnsafeCell<*mut RcBox<T>>,
}

impl<T: 'static> Rc<T> {
    pub fn new(val: T) -> Self {
        let rc_box = RcBox::new(val);
        let rc_box = alloc(rc_box).unwrap_or_else(|| panic!("oom!"));
        Self {
            pointer: UnsafeCell::new(rc_box),
        }
    }
}

impl<T> Clone for Rc<T> {
    fn clone(&self) -> Self {
        unsafe {
            // increment count
            (&mut **self.pointer.get()).count += 1;
            Self {
                pointer: UnsafeCell::new(*self.pointer.get())
            }
        }
    }
}

impl<T> Deref for Rc<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            &(&**self.pointer.get()).value
        }
    }
}

impl<T> Drop for Rc<T> {
    fn drop(&mut self) {
        unsafe {
            (&mut **self.pointer.get()).count -= 1;
            if (&**self.pointer.get()).count == 0 {
                HEAP.as_ref().unwrap().dealloc_object(
                    *self.pointer.get() as *mut u8,
                );
            }
        }
    }
}
