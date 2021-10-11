use super::*;
use drogue_device::{ActorContext, ActorSpawner, Address, Package};
use heapless::Vec;
use nrf_softdevice::ble::{gatt_server, Connection};

type Gatt = GattServer<MicrobitGattServer, Address<'static, TemperatureMonitor>>;

pub struct MicrobitBleService {
    server: MicrobitGattServer,
    controller: ActorContext<'static, BleController>,
    advertiser: ActorContext<'static, BleAdvertiser<Address<'static, Gatt>>>,
    gatt: ActorContext<'static, Gatt>,
    monitor: ActorContext<'static, TemperatureMonitor>,
}

#[nrf_softdevice::gatt_server]
pub struct MicrobitGattServer {
    temperature: TemperatureService,
    device_info: DeviceInformationService,
}

impl MicrobitBleService {
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

impl Package for MicrobitBleService {
    type Primary = BleController;

    fn mount<S: ActorSpawner>(
        &'static self,
        _: Self::Configuration,
        spawner: S,
    ) -> Address<Self::Primary> {
        let controller = self.controller.mount((), spawner);
        let monitor = self.monitor.mount(&self.server.temperature, spawner);
        let acceptor = self.gatt.mount((&self.server, monitor), spawner);
        self.advertiser.mount(acceptor, spawner);
        controller
    }
}

impl GattEventHandler<MicrobitGattServer> for Address<'static, TemperatureMonitor> {
    fn on_event(&mut self, connection: Connection, event: MicrobitGattServerEvent) {
        if let MicrobitGattServerEvent::Temperature(e) = event {
            self.notify((connection, e)).ok();
        }
    }
}
