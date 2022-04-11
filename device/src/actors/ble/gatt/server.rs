use crate::{Actor, Address, Inbox};
use core::future::Future;
use nrf_softdevice::ble::{
    gatt_server::{self, Server},
    Connection,
};

pub struct GattServer<S>
where
    S: Server + 'static,
{
    server: &'static S,
    handler: Address<GattEvent<S>>,
}

pub enum GattEvent<S>
where
    S: Server,
{
    Connected(Connection),
    Write(S::Event),
    Disconnected(Connection),
}

impl<S> GattServer<S>
where
    S: Server,
{
    pub fn new(server: &'static S, handler: Address<GattEvent<S>>) -> Self {
        Self { server, handler }
    }
}

impl<S> Actor for GattServer<S>
where
    S: Server,
{
    type Message<'m> = Connection;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<Connection>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Connection>,
        mut inbox: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Connection> + 'm,
    {
        async move {
            let handler = self.handler.clone();
            loop {
                let conn = inbox.next().await;

                let _ = handler.try_notify(GattEvent::Connected(conn.clone()));

                // Run the GATT server on the connection. This returns when the connection gets disconnected.
                let res = gatt_server::run(&conn, self.server, |e| {
                    trace!("GATT write event received");
                    let _ = handler.try_notify(GattEvent::Write(e));
                })
                .await;

                let _ = handler.try_notify(GattEvent::Disconnected(conn.clone()));

                if let Err(e) = res {
                    info!("gatt_server exited with error: {:?}", e);
                }
            }
        }
    }
}
