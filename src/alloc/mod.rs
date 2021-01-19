use core::mem::{size_of, align_of};
use core::cell::UnsafeCell;
use core::ops::Deref;
use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};
use core::ops::DerefMut;
use crate::alloc::cortex_m::CortexMHeap;
use core::mem;
use crate::alloc::cortex_m::alloc::layout::Layout;

pub mod cortex_m;

pub static mut HEAP: Option<CortexMHeap> = None;

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
    let size = size_of::<T>();
    let size = align_of::<T>();

    unsafe {
        HEAP.as_mut().unwrap().alloc_init(val)
    }
}

#[repr(transparent)]
pub struct Box<T: ?Sized> {
    pointer: UnsafeCell<* mut T>,
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
        log::info!("drop Box");
        unsafe {
            HEAP.as_ref().unwrap().dealloc_object(
                *self.pointer.get() as *mut u8,
            );
        }
    }
}

