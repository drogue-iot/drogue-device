#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]
#![doc = include_str!("../README.md")]
#![warn(missing_docs)]
mod board;
pub use board::*;

pub mod accelerometer;
pub mod display;
pub mod mic;
pub mod speaker;
