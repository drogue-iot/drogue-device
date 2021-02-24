#[cfg(target_arch = "arm")]
pub mod cortex_m;

#[cfg(target_arch = "x86_64")]
pub mod x86;
