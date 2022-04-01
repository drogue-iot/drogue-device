use crate::{Actor, Address, Inbox};
use core::future::Future;
use nrf_softdevice::ble::{
    gatt_server::{self, Server},
    Connection,
};

pub struct GattServer<S, E>
where
    S: Server + 'static,
    E: Actor<Message<'static> = GattEvent<S>> + 'static,
{
    server: &'static S,
    handler: Address<E>,
}

pub enum GattEvent<S>
where
    S: Server,
{
    Connected(Connection),
    Write(S::Event),
    Disconnected(Connection),
}

impl<S, E> GattServer<S, E>
where
    S: Server,
    E: Actor<Message<'static> = GattEvent<S>> + 'static,
{
    pub fn new(server: &'static S, handler: Address<E>) -> Self {
        Self { server, handler }
    }
}

impl<S, E> Actor for GattServer<S, E>
where
    S: Server,
    E: Actor<Message<'static> = GattEvent<S>> + 'static,
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
            let handler = self.handler.clone();
            loop {
                if let Some(mut m) = inbox.next().await {
                    let conn = m.message();

                    let _ = handler
                        .request(GattEvent::Connected(conn.clone()))
                        .unwrap()
                        .await;

                    // Run the GATT server on the connection. This returns when the connection gets disconnected.
                    let res = gatt_server::run(conn, self.server, |e| {
                        trace!("GATT write event received");
                        let _ = handler.notify(GattEvent::Write(e));
                    })
                    .await;

                    let _ = handler
                        .request(GattEvent::Disconnected(conn.clone()))
                        .unwrap()
                        .await;

                    if let Err(e) = res {
                        info!("gatt_server exited with error: {:?}", e);
                    }
                }
            }
        }
    }
}

impl<S, E> super::advertiser::Acceptor for Address<GattServer<S, E>>
where
    S: Server,
    E: Actor<Message<'static> = GattEvent<S>> + 'static,
{
    type Error = ();
    fn accept(&mut self, connection: Connection) -> Result<(), Self::Error> {
        self.notify(connection).map_err(|_| ())
    }
}
