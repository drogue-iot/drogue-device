use core::future::Future;
use drogue_device::{Actor, Address, Inbox};
use nrf_softdevice::ble::{
    gatt_server::{self, Server},
    Connection,
};

mod device_info;
mod temperature;

pub use device_info::*;
pub use temperature::*;

pub struct GattServer<S, E>
where
    S: Server + 'static,
    E: GattEventHandler<S> + 'static,
{
    _m1: core::marker::PhantomData<&'static S>,
    _m2: core::marker::PhantomData<&'static E>,
}

pub enum GattEvent<S>
where
    S: Server,
{
    Connected(Connection),
    Write(Connection, S::Event),
    Disconnected(Connection),
}

pub trait GattEventHandler<S>
where
    S: Server,
{
    fn on_event(&mut self, event: GattEvent<S>);
}

impl<S, E> GattServer<S, E>
where
    S: Server,
    E: GattEventHandler<S>,
{
    pub fn new() -> Self {
        Self {
            _m1: core::marker::PhantomData,
            _m2: core::marker::PhantomData,
        }
    }
}

impl<S, E> Actor for GattServer<S, E>
where
    S: Server,
    E: GattEventHandler<S>,
{
    type Message<'m> = Connection;

    type Configuration = (&'static S, E);

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
        let (server, mut handler) = configuration;
        async move {
            loop {
                loop {
                    if let Some(mut m) = inbox.next().await {
                        let conn = m.message();

                        handler.on_event(GattEvent::Connected(conn.clone()));

                        // Run the GATT server on the connection. This returns when the connection gets disconnected.
                        let res = gatt_server::run(conn, server, |e| {
                            trace!("GATT write event received");
                            handler.on_event(GattEvent::Write(conn.clone(), e));
                        })
                        .await;

                        handler.on_event(GattEvent::Disconnected(conn.clone()));

                        if let Err(e) = res {
                            info!("gatt_server exited with error: {:?}", e);
                        }
                    }
                }
            }
        }
    }
}

impl<S, E> super::advertiser::Acceptor for Address<'static, GattServer<S, E>>
where
    S: Server,
    E: GattEventHandler<S>,
{
    type Error = ();
    fn accept(&mut self, connection: Connection) -> Result<(), Self::Error> {
        self.notify(connection).map_err(|_| ())
    }
}
