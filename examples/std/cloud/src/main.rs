#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::{actors::tcp::std::*, domain::temperature::Celsius, *};
use drogue_temperature::*;
use embassy::time::Duration;
use rand::rngs::OsRng;

pub struct StdBoard;

impl TemperatureBoard for StdBoard {
    type NetworkPackage = ActorContext<'static, StdTcpActor>;
    type Network = StdTcpActor;
    type TemperatureScale = Celsius;
    type SensorReadyIndicator = AlwaysReady;
    type Sensor = FakeSensor;
    type SendTrigger = TimeTrigger;
    type Rng = OsRng;
}

static DEVICE: DeviceContext<TemperatureDevice<StdBoard>> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    DEVICE.configure(TemperatureDevice::new(TemperatureBoardConfig {
        network: ActorContext::new(StdTcpActor::new()),
        send_trigger: TimeTrigger(Duration::from_secs(10)),
        sensor: FakeSensor(22.0),
        sensor_ready: AlwaysReady,
    }));

    DEVICE
        .mount(|device| device.mount(spawner, (), OsRng))
        .await;
}
