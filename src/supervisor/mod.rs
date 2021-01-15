use core::mem::{size_of, align_of, MaybeUninit};
use core::cell::UnsafeCell;
use core::ops::Deref;
use core::pin::Pin;
use core::future::Future;
use core::task::{Context, Poll};
use std::ops::DerefMut;
use std::borrow::BorrowMut;

pub fn alloc<T>(val: T) -> Option<&'static mut T>
{
    let size = size_of::<T>();
    let size = align_of::<T>();

    unimplemented!()
}

pub fn alloc_memory<T>(val: T) -> * mut u8 {
    unimplemented!()
}

pub fn alloc_init<T>(val: T) -> * mut T {
    let memory = alloc_memory(val);
    unimplemented!()
}

pub fn alloc_pinned<T>(val: T) -> Pin<Box<T>> {
    unimplemented!()
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

/*
impl<T> Box<T> {
    pub fn new(val: T) -> Self {
        unimplemented!()
    }

    pub fn into_pin(&self) -> Pin<Self> {
        unimplemented!()

    }
}

 */

/*
impl<T: Future + ?Sized> Future for Box<T> {
    type Output = T::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            T::poll(Pin::new_unchecked(&mut *self), cx)
        }
    }
}

 */

impl<T: Future + ?Sized + 'static> Future for Box<T> {
    type Output = T::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            T::poll(Pin::new_unchecked(&mut *self), cx)
        }
    }
}

pub fn alloc_box<T>(val: T) -> Option<Box<T>> {
    None
    /*
    let b = alloc(val)?;

    Some( Box {
        val: UnsafeCell::new(b)
    } )

     */
}