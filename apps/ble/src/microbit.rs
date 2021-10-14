use super::*;
use drogue_device::{ActorContext, ActorSpawner, Address, Package};
use heapless::Vec;
use nrf_softdevice::ble::gatt_server;

type Gatt<C> = GattServer<MicrobitGattServer, MicrobitGattHandler<C>>;

pub struct MicrobitBleService<C>
where
    C: ConnectionStateListener + 'static,
{
    server: MicrobitGattServer,
    controller: ActorContext<'static, BleController>,
    advertiser: ActorContext<'static, BleAdvertiser<Address<'static, Gatt<C>>>>,
    gatt: ActorContext<'static, Gatt<C>>,
    monitor: ActorContext<'static, TemperatureMonitor>,
}

#[nrf_softdevice::gatt_server]
pub struct MicrobitGattServer {
    temperature: TemperatureService,
    device_info: DeviceInformationService,
}

impl<C> MicrobitBleService<C>
where
    C: ConnectionStateListener,
{
    pub fn new() -> Self {
        let (controller, sd) = BleController::new("Drogue IoT micro:bit v2.0");

        let server: MicrobitGattServer = gatt_server::register(sd).unwrap();

        server
            .device_info
            .model_number_set(Vec::from_slice(b"Drogue IoT micro:bit V2.0").unwrap())
            .unwrap();
        server
            .device_info
            .serial_number_set(Vec::from_slice(b"1").unwrap())
            .unwrap();
        server
            .device_info
            .manufacturer_name_set(Vec::from_slice(b"BBC").unwrap())
            .unwrap();
        server
            .device_info
            .hardware_revision_set(Vec::from_slice(b"1").unwrap())
            .unwrap();

        Self {
            server,
            controller: ActorContext::new(controller),
            advertiser: ActorContext::new(BleAdvertiser::new(sd, "Drogue Low Energy")),
            gatt: ActorContext::new(GattServer::new()),
            monitor: ActorContext::new(TemperatureMonitor::new(sd)),
        }
    }
}

impl<C> Package for MicrobitBleService<C>
where
    C: ConnectionStateListener,
{
    type Primary = BleController;
    type Configuration = C;

    fn mount<S: ActorSpawner>(
        &'static self,
        listener: Self::Configuration,
        spawner: S,
    ) -> Address<Self::Primary> {
        let controller = self.controller.mount((), spawner);
        let monitor = self.monitor.mount(&self.server.temperature, spawner);
        let handler = MicrobitGattHandler {
            temperature: monitor,
            listener,
        };
        let acceptor = self.gatt.mount((&self.server, handler), spawner);
        self.advertiser.mount(acceptor, spawner);
        controller
    }
}

pub trait ConnectionStateListener {
    fn on_connected(&self);
    fn on_disconnected(&self);
}

struct MicrobitGattHandler<C>
where
    C: ConnectionStateListener,
{
    pub temperature: Address<'static, TemperatureMonitor>,
    pub listener: C,
}

impl<C> GattEventHandler<MicrobitGattServer> for MicrobitGattHandler<C>
where
    C: ConnectionStateListener,
{
    fn on_event(&mut self, event: GattEvent<MicrobitGattServer>) {
        match event {
            GattEvent::Write(connection, e) => {
                if let MicrobitGattServerEvent::Temperature(e) = e {
                    self.temperature.notify((connection, e)).ok();
                }
            }
            GattEvent::Connected(_) => self.listener.on_connected(),
            GattEvent::Disconnected(_) => self.listener.on_disconnected(),
        }
    }
}
