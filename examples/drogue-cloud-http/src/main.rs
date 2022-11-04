#![macro_use]
#![feature(type_alias_impl_trait)]

use async_io::Async;
use core::future::Future;
use drogue_device::domain::temperature::Celsius;
use drogue_temperature::*;
use embassy_time::Duration;
use embedded_io::adapters::FromFutures;
use embedded_nal_async::*;
use futures::io::BufReader;
use rand::rngs::OsRng;
use static_cell::StaticCell;
use std::net::TcpStream;

#[drogue_device::main]
async fn main(device: Device, spawner: drogue_device::Spawner) {
    let network = device.network();
}

