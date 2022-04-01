use core::future::Future;
use drogue_device::{Actor, Address, Inbox};
use nrf_softdevice::ble::{
    gatt_server::{self, Server},
    Connection,
};

pub struct GattServer<S, E>
where
    S: Server + 'static,
    E: GattEventHandler<S> + 'static,
{
    server: &'static S,
    handler: E,
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
    type OnEventFuture<'m>: core::future::Future<Output = ()>
    where
        Self: 'm;
    fn on_event<'m>(&'m mut self, event: GattEvent<S>) -> Self::OnEventFuture<'m>;
}

impl<S, E> GattServer<S, E>
where
    S: Server,
    E: GattEventHandler<S>,
{
    pub fn new(server: &'static S, handler: E) -> Self {
        Self { server, handler }
    }
}

impl<S, E> Actor for GattServer<S, E>
where
    S: Server,
    E: GattEventHandler<S>,
{
    type Message<'m> = Connection;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<Self>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            loop {
                loop {
                    if let Some(mut m) = inbox.next().await {
                        let conn = m.message();

                        self.handler.on_event(GattEvent::Connected(conn.clone()));

                        // Run the GATT server on the connection. This returns when the connection gets disconnected.
                        let res = gatt_server::run(conn, self.server, |e| {
                            trace!("GATT write event received");
                            self.handler.on_event(GattEvent::Write(conn.clone(), e));
                        })
                        .await;

                        self.handler.on_event(GattEvent::Disconnected(conn.clone()));

                        if let Err(e) = res {
                            info!("gatt_server exited with error: {:?}", e);
                        }
                    }
                }
            }
        }
    }
}

impl<S, E> super::advertiser::Acceptor for Address<GattServer<S, E>>
where
    S: Server,
    E: GattEventHandler<S>,
{
    type Error = ();
    fn accept(&mut self, connection: Connection) -> Result<(), Self::Error> {
        self.notify(connection).map_err(|_| ())
    }
}
