use core::future::Future;
use drogue_device::{Actor, Address, Inbox};
use nrf_softdevice::ble::{peripheral, Connection};
use nrf_softdevice::{raw, Softdevice};

pub trait Acceptor {
    type Error;
    fn accept(&mut self, connection: Connection) -> Result<(), Self::Error>;
}

pub struct BleAdvertiser<A: Acceptor + 'static> {
    sd: &'static Softdevice,
    _marker: core::marker::PhantomData<&'static A>,
}

impl<A: Acceptor> BleAdvertiser<A> {
    pub fn new(sd: &'static Softdevice) -> Self {
        Self {
            sd,
            _marker: core::marker::PhantomData,
        }
    }
}

impl<A: Acceptor> Actor for BleAdvertiser<A> {
    type Configuration = A;

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where Self: 'm, M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        mut acceptor: Self::Configuration,
        _: Address<'static, Self>,
        _: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        #[rustfmt::skip]
    let adv_data = &[
        0x02, 0x01, raw::BLE_GAP_ADV_FLAGS_LE_ONLY_GENERAL_DISC_MODE as u8,
        0x03, 0x03, 0x09, 0x18,
        0x12, 0x09, b'D', b'r', b'o', b'g', b'u', b'e', b' ', b'L', b'o', b'w', b' ', b'E',b'n', b'e', b'r', b'g', b'y',
    ];
        #[rustfmt::skip]
    let scan_data = &[
        0x03, 0x03, 0x09, 0x18,
    ];
        info!("advertising started!");

        async move {
            loop {
                let config = peripheral::Config::default();
                let adv = peripheral::ConnectableAdvertisement::ScannableUndirected {
                    adv_data,
                    scan_data,
                };
                let conn = peripheral::advertise_connectable(self.sd, adv, &config)
                    .await
                    .unwrap();

                info!("connection established: {}", conn.handle());

                acceptor.accept(conn).ok().unwrap();
            }
        }
    }
}
