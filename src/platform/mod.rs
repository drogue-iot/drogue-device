pub mod cortex_m;

pub use self::cortex_m::{with_critical_section, CriticalSection, Mutex};
