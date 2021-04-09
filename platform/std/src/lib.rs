#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]

pub use drogue_device_kernel::*;
pub use embassy_std;

#[cfg(test)]
mod tests {
    #[test]
    fn test_device_setup() {}
}
