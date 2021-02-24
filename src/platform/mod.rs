#[cfg(target_arch = "arm")]
pub mod cortex_m;

#[cfg(target_arch = "arm")]
pub use self::cortex_m::{exception, with_critical_section, CriticalSection, Mutex};

#[cfg(target_arch = "x86_64")]
pub mod x86;

#[cfg(target_arch = "x86_64")]
pub use self::x86::{exception, with_critical_section, CriticalSection, Mutex};
