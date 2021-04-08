#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]

pub use drogue_device_kernel::*;
pub use drogue_device_macros::{self as drogue};
pub use embassy::time::{Duration, Timer};
pub use embassy::util::Forever;

#[cfg(test)]
mod tests {
    use super::*;
    use log;

    #[test]
    fn test_device_setup() {
    }
}
