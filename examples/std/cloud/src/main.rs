#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use async_io::Async;
use core::future::Future;
use drogue_device::domain::temperature::Celsius;
use drogue_temperature::*;
use embassy_executor::time::Duration;
use embassy_util::Forever;
use embedded_io::adapters::FromFutures;
use embedded_nal_async::*;
use futures::io::BufReader;
use rand::rngs::OsRng;
use std::net::TcpStream;

pub struct StdBoard;

impl TemperatureBoard for StdBoard {
    type Network = TcpClient;
    type TemperatureScale = Celsius;
    type SensorReadyIndicator = AlwaysReady;
    type Sensor = FakeSensor;
    type SendTrigger = TimeTrigger;
    type Rng = OsRng;
}

static DEVICE: Forever<TemperatureDevice<StdBoard>> = Forever::new();

#[embassy_executor::main]
async fn main(spawner: embassy_executor::executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .format_timestamp_nanos()
        .init();

    DEVICE
        .put(TemperatureDevice::new())
        .mount(
            spawner,
            OsRng,
            TemperatureBoardConfig {
                send_trigger: TimeTrigger(Duration::from_secs(10)),
                sensor: FakeSensor(22.0),
                sensor_ready: AlwaysReady,
                network: TcpClient,
            },
        )
        .await;
}

pub struct TcpClient;

impl TcpConnect for TcpClient {
    type Error = std::io::Error;
    type Connection<'m> = FromFutures<BufReader<Async<TcpStream>>>;
    type ConnectFuture<'m> = impl Future<Output = Result<Self::Connection<'m>, Self::Error>> + 'm
    where
        Self: 'm;
    fn connect<'m>(&'m self, remote: SocketAddr) -> Self::ConnectFuture<'m> {
        async move {
            match TcpStream::connect(format!("{}:{}", remote.ip(), remote.port())) {
                Ok(stream) => {
                    let stream = Async::new(stream).unwrap();
                    let stream = futures::io::BufReader::new(stream);
                    let stream = FromFutures::new(stream);
                    Ok(stream)
                }
                Err(e) => Err(e),
            }
        }
    }
}
