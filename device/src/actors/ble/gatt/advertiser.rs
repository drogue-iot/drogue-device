use crate::{Actor, Address, Inbox};
use core::future::Future;
use heapless::Vec;
use nrf_softdevice::ble::peripheral;
pub use nrf_softdevice::ble::Connection;
use nrf_softdevice::{raw, Softdevice};

pub trait Acceptor {
    type Error;
    fn accept(&mut self, connection: Connection) -> Result<(), Self::Error>;
}

pub struct BleAdvertiser<A: Acceptor + 'static> {
    sd: &'static Softdevice,
    name: &'static str,
    acceptor: A,
}

impl<A: Acceptor> BleAdvertiser<A> {
    pub fn new(sd: &'static Softdevice, name: &'static str, acceptor: A) -> Self {
        // Max bytes we have in advertisement packet
        assert!(name.len() < 22);

        Self { sd, name, acceptor }
    }
}

impl<A: Acceptor> Actor for BleAdvertiser<A> {
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<Self::Message<'m>>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self::Message<'m>>,
        _: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        let mut adv_data: Vec<u8, 31> = Vec::new();
        #[rustfmt::skip]
        adv_data.extend_from_slice(&[
            0x02, 0x01, raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8,
            0x03, 0x03, 0x00, 0x61,
            (1 + self.name.len() as u8), 0x09]).unwrap();

        adv_data
            .extend_from_slice(self.name.as_bytes())
            .ok()
            .unwrap();

        #[rustfmt::skip]
        let scan_data = &[
            0x03, 0x03, 0xA, 0x18,
        ];
        debug!("advertising started!");

        async move {
            loop {
                let config = peripheral::Config::default();
                let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
                    adv_data: &adv_data[..],
                    scan_data,
                };
                let conn = peripheral::advertise_connectable(self.sd, adv, &config)
                    .await
                    .unwrap();

                debug!("connection established: {}", conn.handle());

                self.acceptor.accept(conn).ok().unwrap();
            }
        }
    }
}

impl Acceptor for Address<Connection> {
    type Error = ();
    fn accept(&mut self, connection: Connection) -> Result<(), Self::Error> {
        self.try_notify(connection).map_err(|_| ())
    }
}
