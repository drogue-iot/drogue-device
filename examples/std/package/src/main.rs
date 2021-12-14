#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::*;
use embassy::time::{Duration, Timer};

mod package;
use package::*;

pub struct MyDevice {
    package: MyPackage,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner) {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_nanos()
        .init();

    let package = DEVICE
        .configure(MyDevice {
            package: MyPackage::new(),
        })
        .package
        .mount((), spawner);

    // Dispatch increment messages to the package every 2 seconds
    loop {
        package.notify(Increment).unwrap();
        Timer::after(Duration::from_secs(2)).await;
    }
}
