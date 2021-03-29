pub(crate) mod actor;
pub(crate) mod address;
pub(crate) mod bus;
pub(crate) mod device;
pub(crate) mod handler;
pub(crate) mod interrupt;
pub(crate) mod macros;
pub(crate) mod package;
pub(crate) mod supervisor;

pub use device::{Device, DeviceConfiguration, DeviceContext};

//use crate::arena::define_arena;
//define_arena!(SystemArena);
