//use crate::platform::alloc::layout::Layout;
//use crate::platform::cortex_m_alloc::CortexMHeap;

use drogue_arena::{Arena, Layout};
use drogue_arena::{
    define_arena,
    init_arena,
};

use drogue_tls_sys::{platform_set_calloc_free, platform_set_vsnprintf, platform_set_snprintf};
use drogue_tls_sys::types::{c_char, c_int};
use core::ffi::c_void;
use crate::entropy::EntropyContext;
use crate::rng::ctr_drbg::CtrDrbgContext;
use crate::ssl::config::{SslConfig, Transport, Preset};

use drogue_ffi_compat::{vsnprintf, snprintf};
use core::mem::size_of;

//static mut ALLOCATOR: Option<CortexMHeap> = Option::None;

pub struct SslPlatform {
    entropy_context: EntropyContext,
    ctr_drbg_context: CtrDrbgContext,
}

extern "C" {
    fn platform_snprintf(s: *mut c_char,
                         n: usize,
                         format: *const c_char,
                         ...) -> c_int;
}

define_arena!(TlsArena);

macro_rules! init_tls_arena {
    ($size:literal) => {
        drogue_arena::init_arena!($crate::platform | TlsArena => $size);
    }
}

impl SslPlatform {
    pub fn setup(start: usize, size: usize) -> Option<Self> {
        unsafe {
            platform_set_vsnprintf(Some(vsnprintf));
            platform_set_snprintf(Some(snprintf));
        }
        unsafe { platform_set_calloc_free(Some(platform_calloc_f), Some(platform_free_f)) };
        Some(Self {
            entropy_context: EntropyContext::default(),
            ctr_drbg_context: CtrDrbgContext::default(),
        })
    }

    pub fn entropy_context(&self) -> &EntropyContext {
        &self.entropy_context
    }

    pub fn entropy_context_mut(&mut self) -> &mut EntropyContext {
        &mut self.entropy_context
    }

    pub fn seed_rng(&mut self) -> Result<(), ()> {
        self.ctr_drbg_context.seed(&mut self.entropy_context)
    }

    pub fn new_client_config(&mut self, transport: Transport, preset: Preset) -> Result<SslConfig, ()> {
        let mut config = SslConfig::client(transport, preset)?;
        config.rng(&mut self.ctr_drbg_context);
        Ok(config)
    }
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub extern "C" fn strlen(p: *const c_char) -> usize {
    let mut n = 0;
    unsafe {
        while *p.add(n) != 0 {
            n += 1;
        }
    }
    n
}

extern "C" fn platform_calloc_f(count: usize, size: usize) -> *mut c_void {
    let requested_size = count * size;
    let header_size = 2 * size_of::<usize>();
    let total_size = header_size + requested_size;
    let layout = Layout::from_size_align(total_size, 4)
        .unwrap()
        .pad_to_align();

    unsafe {
        let mut ptr = TlsArena::alloc_by_layout(layout, true);
        if ptr.is_null() {
            log::error!("failed to allocate {} from {}", requested_size, TlsArena::info().free);
            return core::ptr::null_mut();
        }
        let mut ptr = ptr as *mut usize;
        *ptr = layout.size();
        ptr = ptr.add(1);
        *ptr = layout.align();
        ptr = ptr.add(1);
        ptr as *mut c_void
    }
}

extern "C" fn platform_free_f(ptr: *mut c_void) {
    if ptr as u32 == 0 {
        return;
    }

    unsafe {
        let mut ptr = ptr as *mut usize;
        ptr = ptr.offset(-1);
        let align = *ptr;
        ptr = ptr.offset(-1);
        let size = *ptr;
        TlsArena::dealloc_by_layout(ptr as *mut u8, Layout::from_size_align(size, align).unwrap())
    }
}

