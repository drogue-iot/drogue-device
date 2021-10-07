use core::future::Future;
use drogue_device::{Actor, Address, Inbox};
use nrf_softdevice::ble::{
    gatt_server::{self, RegisterError, Server},
    Connection,
};
use nrf_softdevice::Softdevice;

mod temperature;
mod device_info;

pub use temperature::*;
pub use device_info::*;

pub struct GattServer {
    sd: &'static Softdevice,
}

pub enum GattServerEvent {
    NewConnection(Connection),
}

impl GattServer {
    pub fn new(sd: &'static Softdevice) -> Self {
        Self { sd }
    }

    pub fn register<S: Server>(&mut self) -> Result<S, RegisterError> {
        gatt_server::register(self.sd)
    }
}

impl Actor for GattServer {
    type Message<'m> = GattServerEvent;

    type Configuration = (
        &'static TemperatureService,
        Address<'static, TemperatureMonitor>,
    );

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where Self: 'm, M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        configuration: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        let (service, monitor) = configuration;
        async move {
            loop {
                loop {
                    if let Some(mut m) = inbox.next().await {
                        let GattServerEvent::NewConnection(conn) = m.message();
                        // Run the GATT server on the connection. This returns when the connection gets disconnected.
                        let res = gatt_server::run(conn, |e| {
                            if let Some(e) = service.on_write(e) {
                                monitor.notify((conn.clone(), e)).unwrap();
                            }
                        })
                        .await;

                        if let Err(e) = res {
                            info!("gatt_server exited with error: {:?}", e);
                        }
                    }
                }
            }
        }
    }
}

impl super::advertiser::Acceptor for Address<'static, GattServer> {
    type Error = ();
    fn accept(&mut self, connection: Connection) -> Result<(), Self::Error> {
        self.notify(GattServerEvent::NewConnection(connection))
            .map_err(|_| ())
    }
}
