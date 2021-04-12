#![macro_use]
#![cfg_attr(not(feature = "std"), no_std)]
#![allow(incomplete_features)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

pub(crate) mod fmt;

pub mod actor;
pub mod channel;
pub mod device;
pub mod signal;

pub use actor::{Actor, ActorState, Address};
pub use channel::{consts, Channel};
pub use device::{Device, DeviceContext, DeviceMounter};
pub use drogue_device_macros::*;
pub use embassy;
pub use embassy::executor::{raw::Task, SpawnToken, Spawner};
pub use embassy::task;
pub use embassy::time::{Duration, Timer};
pub use embassy::util::Forever;
