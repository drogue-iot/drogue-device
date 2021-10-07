use super::*;
use drogue_device::{ActorContext, ActorSpawner, Address, Package};
use heapless::Vec;

pub struct MicrobitBleService {
    temperature_service: TemperatureService,
    _device_info_service: DeviceInformationService,
    controller: ActorContext<'static, BleController>,
    advertiser: ActorContext<'static, BleAdvertiser<Address<'static, GattServer>>>,
    gatt: ActorContext<'static, GattServer>,
    monitor: ActorContext<'static, TemperatureMonitor>,
}

impl MicrobitBleService {
    pub fn new() -> Self {
        let (controller, sd) = BleController::new("Drogue IoT micro:bit v2.0");

        let mut gatt = GattServer::new(sd);
        let device_info: DeviceInformationService = gatt.register().unwrap();
        device_info
            .model_number_set(Vec::from_slice(b"Drogue IoT micro:bit V2.0").unwrap())
            .unwrap();
        device_info
            .serial_number_set(Vec::from_slice(b"1").unwrap())
            .unwrap();
        device_info
            .manufacturer_name_set(Vec::from_slice(b"BBC").unwrap())
            .unwrap();
        device_info
            .hardware_revision_set(Vec::from_slice(b"1").unwrap())
            .unwrap();

        Self {
            temperature_service: gatt.register().unwrap(),
            _device_info_service: device_info,
            controller: ActorContext::new(controller),
            advertiser: ActorContext::new(BleAdvertiser::new(sd, "Drogue Low Energy")),
            gatt: ActorContext::new(gatt),
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
        let monitor = self.monitor.mount(&self.temperature_service, spawner);
        let acceptor = self
            .gatt
            .mount((&self.temperature_service, monitor), spawner);
        self.advertiser.mount(acceptor, spawner);
        controller
    }
}
