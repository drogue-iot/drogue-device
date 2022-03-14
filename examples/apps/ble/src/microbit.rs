use super::*;
use drogue_device::{ActorContext, ActorSpawner, Address, Package};
use heapless::Vec;
use nrf_softdevice::{ble::gatt_server, Softdevice};

type Gatt<C> = GattServer<MicrobitGattServer, MicrobitGattHandler<C>>;

pub struct MicrobitBleService<C>
where
    C: ConnectionStateListener + 'static,
{
    sd: &'static Softdevice,
    server: MicrobitGattServer,
    controller: ActorContext<BleController>,
    advertiser: ActorContext<BleAdvertiser<Address<Gatt<C>>>>,
    gatt: ActorContext<Gatt<C>>,
    monitor: ActorContext<TemperatureMonitor>,
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
        let sd = BleController::new_sd("Drogue IoT micro:bit v2.0");
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
            sd,
            server,
            controller: ActorContext::new(),
            advertiser: ActorContext::new(),
            gatt: ActorContext::new(),
            monitor: ActorContext::new(),
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
        let controller = self.controller.mount(spawner, BleController::new(self.sd));
        let monitor = self.monitor.mount(
            spawner,
            TemperatureMonitor::new(self.sd, &self.server.temperature),
        );
        let handler = MicrobitGattHandler {
            temperature: monitor,
            listener,
        };
        let acceptor = self
            .gatt
            .mount(spawner, GattServer::new(&self.server, handler));
        self.advertiser.mount(
            spawner,
            BleAdvertiser::new(self.sd, "Drogue Low Energy", acceptor),
        );
        controller
    }
}

pub trait ConnectionStateListener {
    type OnConnectedFuture<'m>: core::future::Future<Output = ()>
    where
        Self: 'm;

    fn on_connected<'m>(&'m self) -> Self::OnConnectedFuture<'m>;

    type OnDisconnectedFuture<'m>: core::future::Future<Output = ()>
    where
        Self: 'm;
    fn on_disconnected<'m>(&'m self) -> Self::OnDisconnectedFuture<'m>;
}

struct MicrobitGattHandler<C>
where
    C: ConnectionStateListener,
{
    pub temperature: Address<TemperatureMonitor>,
    pub listener: C,
}

impl<C> GattEventHandler<MicrobitGattServer> for MicrobitGattHandler<C>
where
    C: ConnectionStateListener,
{
    type OnEventFuture<'m> = impl core::future::Future<Output = ()> + 'm
    where
        C: 'm;
    fn on_event<'m>(&'m mut self, event: GattEvent<MicrobitGattServer>) -> Self::OnEventFuture<'m> {
        async move {
            match event {
                GattEvent::Write(connection, e) => {
                    if let MicrobitGattServerEvent::Temperature(e) = e {
                        self.temperature.request((connection, e)).unwrap().await;
                    }
                }
                GattEvent::Connected(_) => self.listener.on_connected().await,
                GattEvent::Disconnected(_) => self.listener.on_disconnected().await,
            }
        }
    }
}
