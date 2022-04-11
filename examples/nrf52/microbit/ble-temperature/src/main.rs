#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

use drogue_device::{
    actors::ble::gatt::{advertiser::*, server::*, temperature::*},
    bsp::boards::nrf52::microbit::{LedMatrix, Microbit},
    drivers::ble::gatt::{device_info::*, enable_softdevice, temperature::*},
    drivers::led::matrix::fonts,
    traits::led::ToFrame,
    Address, Board, *,
};
use embassy::executor::Spawner;
use embassy::time::Duration;
use embassy::util::Forever;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::Peripherals;
use nrf_softdevice::{ble::gatt_server, Softdevice};

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(not(feature = "panic-probe"))]
use panic_reset as _;

// Application must run at a lower priority than softdevice
fn config() -> Config {
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}

#[embassy::main(config = "config()")]
async fn main(spawner: Spawner, p: Peripherals) {
    let board = Microbit::new(p);
    let sd = enable_softdevice("Drogue Device Temperature");

    let mut matrix = board.display;
    matrix.scroll("Hello, Drogue").await;

    static SERVER: Forever<MicrobitGattServer> = Forever::new();
    let server = SERVER.put(gatt_server::register(sd).unwrap());
    server
        .device_info
        .initialize(b"Drogue IoT micro:bit", b"1", b"BBC", b"1")
        .unwrap();

    let temperature = TemperatureMonitor::new(sd, &server.temperature);
    let temperature = spawn_actor!(spawner, MONITOR, TemperatureMonitor, temperature);

    let handler = MicrobitGattHandler {
        temperature,
        matrix,
    };
    let handler = spawn_actor!(spawner, GATT_HANDLER, MicrobitGattHandler, handler);

    type GS = GattServer<MicrobitGattServer>;
    let acceptor = spawn_actor!(spawner, GATT_SERVER, GS, GS::new(server, handler));

    spawn_actor!(
        spawner,
        ADVERTISER,
        BleAdvertiser<Address<Connection>>,
        BleAdvertiser::new(sd, "Drogue Temperature", acceptor)
    );
}

#[nrf_softdevice::gatt_server]
pub struct MicrobitGattServer {
    temperature: TemperatureService,
    device_info: DeviceInformationService,
}

// Handler that gets the dispatched GATT events
struct MicrobitGattHandler {
    temperature: Address<MonitorEvent>,
    matrix: LedMatrix,
}

#[actor]
impl Actor for MicrobitGattHandler {
    type Message<'m> = GattEvent<MicrobitGattServer>;

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
    where
        M: Inbox<Self::Message<'m>>,
    {
        loop {
            match inbox.next().await {
                GattEvent::Write(e) => {
                    if let MicrobitGattServerEvent::Temperature(e) = e {
                        self.temperature.notify(MonitorEvent::Event(e)).await;
                    }
                }
                GattEvent::Connected(c) => {
                    self.temperature
                        .notify(MonitorEvent::AddConnection(c))
                        .await;
                    self.matrix
                        .display(fonts::CHECK_MARK.to_frame(), Duration::from_secs(2))
                        .await;
                }
                GattEvent::Disconnected(c) => {
                    self.temperature
                        .notify(MonitorEvent::RemoveConnection(c))
                        .await;
                    self.matrix.scroll("Disconnected").await;
                }
            }
        }
    }
}

#[embassy::task]
async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}
