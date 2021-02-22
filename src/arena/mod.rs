use crate::arena::static_arena::StaticArena;
use core::cell::UnsafeCell;
use core::fmt::{Debug, Formatter};
use core::future::Future;
use core::marker::PhantomData;
use core::ops::Deref;
use core::ops::DerefMut;
use core::pin::Pin;
use core::ptr::drop_in_place;
use core::task::{Context, Poll};

pub mod static_arena;

pub struct Info {
    pub used: usize,
    pub free: usize,
    pub high_watermark: usize,
}

impl Info {
    pub fn new(heap: &StaticArena) -> Self {
        Self {
            used: heap.used(),
            free: heap.free(),
            high_watermark: heap.high_watermark(),
        }
    }
}

//pub static mut HEAP: Option<StaticArena> = None;

pub trait Arena: Sized {
    fn alloc<'o, T: 'o>(val: T) -> Option<&'o mut T>;
    fn dealloc(ptr: *mut u8);
    fn info() -> Info;
}

#[doc(hidden)]
#[macro_export]
macro_rules! define_arena {
    ($id:ident) => {
        pub struct $id;

        $crate::paste::paste! {
            pub static mut [< $id ARENA >]: Option<$crate::arena::static_arena::StaticArena> = None;

            impl $crate::arena::Arena for $id {
                fn alloc<'o, T: 'o>(val: T) -> Option<&'o mut T> {
                    unsafe { [< $id ARENA >].as_mut().unwrap().alloc_init(val) }
                }

                #[allow(clippy::not_unsafe_ptr_arg_deref)]
                fn dealloc(ptr: *mut u8) {
                    unsafe {
                        [< $id ARENA > ].as_ref()
                            .unwrap()
                            .dealloc_object(ptr);
                    }
                }

                fn info() -> $crate::arena::Info {
                    unsafe {
                        $crate::arena::Info::new( [< $id ARENA >].as_ref().unwrap() )
                    }
                }
            }
            impl Unpin for $id {}
        }
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! init_arena {
    ($mod:path | $id:ident => $size:literal) => {
        $crate::paste::paste! {
            static mut [< $id MEMORY >]: [u8; $size] = [0; $size];
            unsafe {
                $mod::[< $id ARENA >].replace($crate::arena::static_arena::StaticArena::new(&[< $id MEMORY >]));
            }
        }
    }
}

#[repr(transparent)]
pub struct Box<T: ?Sized, A: Arena> {
    pub(crate) pointer: UnsafeCell<*mut T>,
    arena: PhantomData<A>,
}

impl<T: ?Sized, A: Arena> Box<T, A> {
    pub fn new(val: &mut T) -> Self {
        Self {
            pointer: UnsafeCell::new(val),
            arena: PhantomData,
        }
    }
}

impl<T: ?Sized, A: Arena> Deref for Box<T, A> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &**self.pointer.get() }
    }
}

impl<T: ?Sized, A: Arena> DerefMut for Box<T, A> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut **self.pointer.get() }
    }
}

//impl<T: Future + ?Sized + 'static> Future for Box<T> {
impl<T: Future + ?Sized, A: Arena + Unpin> Future for Box<T, A> {
    type Output = T::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        //let t = self.get_mut();
        //unsafe { T::poll(Pin::new_unchecked(t), cx) }
        unsafe { T::poll(Pin::new_unchecked(&mut **self), cx) }
    }
}

impl<T: ?Sized, A: Arena> Drop for Box<T, A> {
    fn drop(&mut self) {
        unsafe {
            drop_in_place(*self.pointer.get());
            A::dealloc(*self.pointer.get() as *mut u8);
        }
    }
}

impl<T: ?Sized + Debug, A: Arena> Debug for Box<T, A> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        //unsafe { (&(*self.pointer.get() as *const T)).fmt(f) }
        Debug::fmt(unsafe { &**self.pointer.get() as &T }, f)
    }
}

struct RcBox<T: ?Sized, A: Arena> {
    count: u8,
    arena: PhantomData<A>,
    value: T,
}

impl<T, A: Arena> RcBox<T, A> {
    pub fn new(value: T) -> Self {
        Self {
            count: 1,
            value,
            arena: PhantomData,
        }
    }
}

pub struct Rc<T, A: Arena> {
    pointer: UnsafeCell<*mut RcBox<T, A>>,
}

impl<'m, T: 'm, A: Arena> Rc<T, A> {
    pub fn new(val: T) -> Self {
        let rc_box = RcBox::new(val);
        let rc_box = A::alloc(rc_box).unwrap_or_else(|| panic!("oom!"));
        Self {
            pointer: UnsafeCell::new(rc_box),
        }
    }
}

impl<T, A: Arena> Clone for Rc<T, A> {
    fn clone(&self) -> Self {
        unsafe {
            // increment count
            (**self.pointer.get()).count += 1;
            Self {
                pointer: UnsafeCell::new(*self.pointer.get()),
            }
        }
    }
}

impl<T, A: Arena> Deref for Rc<T, A> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &(**self.pointer.get()).value }
    }
}

impl<T, A: Arena> Drop for Rc<T, A> {
    fn drop(&mut self) {
        unsafe {
            (**self.pointer.get()).count -= 1;
            if (**self.pointer.get()).count == 0 {
                drop_in_place(*self.pointer.get());
                A::dealloc(*self.pointer.get() as *mut u8);
                //HEAP.as_ref()
                //.unwrap()
                //.dealloc_object(*self.pointer.get() as *mut u8);
            }
        }
    }
}
