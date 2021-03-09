//! A heap allocator for Cortex-M processors.
//!
//! Note that using this as your global allocator requires nightly Rust.
//!
//! # Example
//!
//! For a usage example, see `examples/global_alloc.rs`.

pub mod heap;

use core::cell::RefCell;
//use core::heap::Layout;
use core::ptr::NonNull;

use core::mem;
use core::sync::atomic::{AtomicUsize, Ordering};
use heap::layout::Layout;
use heap::Heap;
//use static_arena::interrupt::Mutex;
use drogue_arch::{with_critical_section, Mutex};

pub struct StaticArena {
    heap: Mutex<RefCell<Heap>>,
    high_watermark: AtomicUsize,
}

impl StaticArena {
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
        with_critical_section(|cs| {
            self.heap.borrow(cs).borrow_mut().init(start_addr, size);
        });
    }

     */

    pub fn new(memory: &'static [u8]) -> Self {
        Self {
            heap: Mutex::new(RefCell::new(Heap::new(memory))),
            high_watermark: AtomicUsize::new(0),
        }

        //with_critical_section(|cs| {
        //heap.heap.borrow(cs).borrow_mut().new(memory);
        //});

        //heap
    }

    /// Returns an estimate of the amount of bytes in use.
    pub fn used(&self) -> usize {
        with_critical_section(|cs| self.heap.borrow(cs).borrow_mut().used())
    }

    /// Returns an estimate of the amount of bytes available.
    pub fn free(&self) -> usize {
        with_critical_section(|cs| self.heap.borrow(cs).borrow_mut().free())
    }

    pub fn high_watermark(&self) -> usize {
        self.high_watermark.load(Ordering::Acquire)
    }

    pub unsafe fn alloc_by_layout(&mut self, layout: Layout, zero: bool) -> *mut u8 {
        let ptr = self.alloc(layout);
        if zero {
            let mut zeroing = ptr as *mut u8;
            for _ in 0..layout.size() {
                zeroing.write(0);
                zeroing = zeroing.add(1);
            }
        }
        ptr
    }

    pub unsafe fn dealloc_by_layout(&mut self, ptr: *mut u8, layout: Layout) {
        self.dealloc(ptr, layout);
    }

    pub fn alloc_init<'o, T: 'o>(&mut self, val: T) -> Option<&'o mut T> {
        let layout = Layout::from_size_align(
            mem::size_of::<(Layout, T)>(),
            mem::align_of::<(Layout, T)>(),
        )
        .unwrap();
        log::trace!(
            "[ALLOC] asking for {} aligned {}",
            layout.size(),
            layout.align()
        );
        unsafe {
            let mut allocation = self.alloc(layout);
            if allocation.is_null() {
                log::warn!(
                    "[ALLOC] allocation failed: requested={}; free={}",
                    layout.size(),
                    self.free()
                );
                None
            } else {
                //let mut allocation = &mut *(allocation as *mut MaybeUninit<T>);
                //allocation.as_mut_ptr().write( (layout, val ));
                //Some(&mut *allocation.as_mut_ptr())
                log::trace!(
                    "[ALLOC] {:x} allocate {} || {} free",
                    allocation as u32,
                    layout.size(),
                    self.free()
                );
                (allocation as *mut Layout).write(layout);
                allocation = (allocation as *mut Layout).add(1) as *mut u8;
                (allocation as *mut T).write(val);
                let used = self.used();
                let high = self.high_watermark.load(Ordering::Acquire);
                if used > high {
                    self.high_watermark.store(used, Ordering::Release);
                }
                Some(&mut *(allocation as *mut _ as *mut T))
                //Some(&*allocation as *mut T)
            }
        }
    }

    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        with_critical_section(|cs| {
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

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        with_critical_section(|cs| {
            self.heap
                .borrow(cs)
                .borrow_mut()
                .deallocate(NonNull::new_unchecked(ptr), layout)
        });
    }

    pub unsafe fn dealloc_object(&self, ptr: *mut u8) {
        let head_ptr = (ptr as *mut Layout).sub(1);
        let layout = head_ptr.read();
        log::trace!(
            "[ALLOC] {:x} deallocate {} || {} free",
            head_ptr as u32,
            layout.size(),
            self.free()
        );
        self.dealloc(head_ptr as *mut u8, head_ptr.read());
    }
}
