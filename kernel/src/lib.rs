#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

mod actor;
mod channel;
mod device;
mod signal;

pub use actor::{Actor, ActorState, Address};
pub use channel::{consts, Channel};
pub use device::{Device, DeviceContext};
pub use drogue_device_macros::{main, Device};
pub use embassy;
pub use embassy::executor::raw::Task;
pub use embassy::time::{Duration, Timer};
pub use embassy::util::Forever;
