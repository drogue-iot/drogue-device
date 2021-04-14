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
