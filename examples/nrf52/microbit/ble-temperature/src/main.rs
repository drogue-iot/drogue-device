#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

use drogue_device::{
    actors::ble::gatt::{advertiser::*, server::*, temperature::*},
    actors::led::matrix::MatrixCommand,
    bsp::boards::nrf52::microbit::{LedMatrixActor, Microbit},
    drivers::ble::gatt::{device_info::*, enable_softdevice, temperature::*},
    drivers::led::matrix::fonts,
    traits::led::TextDisplay,
    Address, Board, *,
};
use embassy::executor::Spawner;
use embassy::util::Forever;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::Peripherals;
use nrf_softdevice::{ble::gatt_server, Softdevice};

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(not(feature = "panic-probe"))]
use panic_reset as _;

pub struct LedConnectionState(Address<LedMatrixActor>);

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

    let mut matrix = spawn_actor!(
        spawner,
        LED_MATRIX,
        LedMatrixActor,
        LedMatrixActor::new(board.display, None)
    );

    matrix.scroll("Hello, Drogue").await.unwrap();

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

    type GS = GattServer<MicrobitGattServer, MicrobitGattHandler>;
    let acceptor = spawn_actor!(spawner, GATT_SERVER, GS, GS::new(server, handler));

    spawn_actor!(
        spawner,
        ADVERTISER,
        BleAdvertiser<Address<GS>>,
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
    temperature: Address<TemperatureMonitor>,
    matrix: Address<LedMatrixActor>,
}

#[actor]
impl Actor for MicrobitGattHandler {
    type Message<'m> = GattEvent<MicrobitGattServer>;

    async fn on_mount<M>(&mut self, _: Address<Self>, inbox: &mut M)
    where
        M: Inbox<Self>,
    {
        loop {
            if let Some(mut m) = inbox.next().await {
                match m.message() {
                    GattEvent::Write(e) => {
                        if let MicrobitGattServerEvent::Temperature(e) = e {
                            self.temperature
                                .request(MonitorEvent::Event(e))
                                .unwrap()
                                .await;
                        }
                    }
                    GattEvent::Connected(c) => {
                        self.temperature
                            .request(MonitorEvent::AddConnection(c))
                            .unwrap()
                            .await;
                        let _ = self
                            .matrix
                            .notify(MatrixCommand::ApplyFrame(&fonts::CHECK_MARK));
                    }
                    GattEvent::Disconnected(c) => {
                        self.temperature
                            .request(MonitorEvent::RemoveConnection(c))
                            .unwrap()
                            .await;
                        self.matrix.scroll("Disconnected").await.unwrap();
                    }
                }
            }
        }
    }
}

#[embassy::task]
async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}
