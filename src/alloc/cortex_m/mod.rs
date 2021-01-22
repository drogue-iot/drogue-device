
//! A heap allocator for Cortex-M processors.
//!
//! Note that using this as your global allocator requires nightly Rust.
//!
//! # Example
//!
//! For a usage example, see `examples/global_alloc.rs`.

pub(crate) mod alloc;

use core::cell::RefCell;
//use core::alloc::Layout;
use core::ptr::NonNull;

use alloc::layout::Layout;
use alloc::Heap;
use cortex_m::interrupt::Mutex;
use core::mem;

pub struct CortexMHeap {
    heap: Mutex<RefCell<Heap>>,
}

impl CortexMHeap {
    /*
    /// Crate a new UNINITIALIZED heap allocator
    ///
    /// You must initialize this heap using the
    /// [`init`](struct.CortexMHeap.html#method.init) method before using the allocator.
    pub fn empty() -> CortexMHeap {
        CortexMHeap {
            heap: Mutex::new(RefCell::new(Heap::empty())),
        }
    }

    /// Initializes the heap
    ///
    /// This function must be called BEFORE you run any code that makes use of the
    /// allocator.
    ///
    /// `start_addr` is the address where the heap will be located.
    ///
    /// `size` is the size of the heap in bytes.
    ///
    /// Note that:
    ///
    /// - The heap grows "upwards", towards larger addresses. Thus `end_addr` must
    ///   be larger than `start_addr`
    ///
    /// - The size of the heap is `(end_addr as usize) - (start_addr as usize)`. The
    ///   allocator won't use the byte at `end_addr`.
    ///
    /// # Unsafety
    ///
    /// Obey these or Bad Stuff will happen.
    ///
    /// - This function must be called exactly ONCE.
    /// - `size > 0`
    pub unsafe fn init(&self, start_addr: usize, size: usize) {
        cortex_m::interrupt::free(|cs| {
            self.heap.borrow(cs).borrow_mut().init(start_addr, size);
        });
    }

     */

    pub fn new(memory: &'static [u8]) -> Self {
        Self {
            heap: Mutex::new(RefCell::new(Heap::new(memory))),
        }

        //cortex_m::interrupt::free(|cs| {
        //heap.heap.borrow(cs).borrow_mut().new(memory);
        //});

        //heap
    }

    /// Returns an estimate of the amount of bytes in use.
    pub fn used(&self) -> usize {
        cortex_m::interrupt::free(|cs| self.heap.borrow(cs).borrow_mut().used())
    }

    /// Returns an estimate of the amount of bytes available.
    pub fn free(&self) -> usize {
        cortex_m::interrupt::free(|cs| self.heap.borrow(cs).borrow_mut().free())
    }

    pub(crate) fn alloc_init<T>(&mut self, val: T) -> Option<&'static mut T> {
        let layout = Layout::from_size_align(mem::size_of::<(Layout,T)>(), mem::align_of::<(Layout,T)>()).unwrap();
        unsafe {
            let mut allocation = self.alloc(layout);
            if allocation.is_null() {
                None
            } else {
                //let mut allocation = &mut *(allocation as *mut MaybeUninit<T>);
                //allocation.as_mut_ptr().write( (layout, val ));
                //Some(&mut *allocation.as_mut_ptr())
                log::trace!("[ALLOC] {:x} allocate {}", allocation as u32, layout.size() );
                (allocation as *mut Layout).write(layout);
                allocation = (allocation as *mut Layout).add(1) as *mut u8;
                (allocation as *mut T).write(val);
                Some( &mut *(allocation as *mut _ as *mut T))
                //Some(&*allocation as *mut T)
            }
        }
    }

    pub unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        cortex_m::interrupt::free(|cs| {
            self.heap
                .borrow(cs)
                .borrow_mut()
                .allocate_first_fit(layout)
                .ok()
                .map_or(core::ptr::null_mut::<u8>(), |allocation| {
                    allocation.as_ptr()
                })
        })
    }

    pub unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        cortex_m::interrupt::free(|cs| {
            self.heap
                .borrow(cs)
                .borrow_mut()
                .deallocate(NonNull::new_unchecked(ptr), layout)
        });
    }

    pub unsafe fn dealloc_object(&self, ptr: *mut u8) {
        let head_ptr = (ptr as *mut Layout).sub( 1 );
        let layout = head_ptr.read();
        log::trace!("[ALLOC] {:x} deallocate {} || {}", head_ptr as u32, layout.size(), self.free() );
        self.dealloc( head_ptr as *mut u8, head_ptr.read());

    }
}