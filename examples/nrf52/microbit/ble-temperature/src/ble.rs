use core::future::Future;
use core::mem;
use drogue_device::{Actor, Address, Inbox};
use fixed::types::I30F2;
use nrf_softdevice::ble::{
    gatt_server::{self, GattEvent, Server},
    peripheral, Connection,
};
use nrf_softdevice::{raw, temperature_celsius, Softdevice};

use embassy::time::Duration;
use heapless::Vec;

use embassy::time::Ticker;
use futures::{future::select, future::Either, pin_mut, Stream, StreamExt};

pub struct BleController {
    pub sd: &'static Softdevice,
}

impl BleController {
    pub fn new(device_name: &'static str) -> (Self, &'static Softdevice) {
        let config = nrf_softdevice::Config {
            clock: Some(raw::nrf_clock_lf_cfg_t {
                source: raw::NRF_CLOCK_LF_SRC_RC as u8,
                rc_ctiv: 4,
                rc_temp_ctiv: 2,
                accuracy: 7,
            }),
            conn_gap: Some(raw::ble_gap_conn_cfg_t {
                conn_count: 6,
                event_length: 24,
            }),
            conn_gatt: Some(raw::ble_gatt_conn_cfg_t { att_mtu: 256 }),
            gatts_attr_tab_size: Some(raw::ble_gatts_cfg_attr_tab_size_t {
                attr_tab_size: 32768,
            }),
            gap_role_count: Some(raw::ble_gap_cfg_role_count_t {
                adv_set_count: 1,
                periph_role_count: 3,
            }),
            gap_device_name: Some(raw::ble_gap_cfg_device_name_t {
                p_value: device_name.as_ptr() as *const u8 as _,
                current_len: device_name.len() as u16,
                max_len: device_name.len() as u16,
                write_perm: unsafe { mem::zeroed() },
                _bitfield_1: raw::ble_gap_cfg_device_name_t::new_bitfield_1(
                    raw::BLE_GATTS_VLOC_STACK as u8,
                ),
            }),
            ..Default::default()
        };
        let sd = Softdevice::enable(&config);
        (Self { sd }, sd)
    }
}

impl Actor for BleController {
    #[rustfmt::skip]
    type OnMountFuture<'m, M> where Self: 'm, M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Self::Configuration,
        _: Address<'static, Self>,
        _: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            self.sd.run().await;
        }
    }
}

//let temperature = TEMPERATURE.put(gatt_server::register(sd).unwrap());

#[nrf_softdevice::gatt_server(uuid = "e95d6100-251d-470a-a062-fa1922dfa9a8")]
pub struct TemperatureService {
    #[characteristic(uuid = "e95d9250-251d-470a-a062-fa1922dfa9a8", read, notify)]
    temperature: i8,
    #[characteristic(uuid = "e95d1b25-251d-470a-a062-fa1922dfa9a8", read, write)]
    period: u16,
}

#[nrf_softdevice::gatt_server(uuid = "0000180a-0000-1000-8000-00805f9b34fb")]
struct DeviceInformationService {
    #[characteristic(uuid = "00002a24-0000-1000-8000-00805f9b34fb", read)]
    model_number: Vec<u8, 32>,
    #[characteristic(uuid = "00002a25-0000-1000-8000-00805f9b34fb", read)]
    serial_number: Vec<u8, 32>,
    #[characteristic(uuid = "00002a27-0000-1000-8000-00805f9b34fb", read)]
    hardware_revision: Vec<u8, 4>,
    #[characteristic(uuid = "00002a29-0000-1000-8000-00805f9b34fb", read)]
    manufacturer_name: Vec<u8, 32>,
}

pub struct GattServer {}

pub enum GattServerEvent {
    NewConnection(Connection),
}

impl GattServer {
    pub fn new() -> Self {
        //    let temperature = gatt_server::register(sd).unwrap();
        //    let device_info = gatt_server::register(sd).unwrap();
        Self {}
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
                                monitor
                                    .notify(TemperatureMonitorEvent(conn.clone(), e))
                                    .unwrap();
                            }
                        })
                        .await;

                        if let Err(e) = res {
                            defmt::info!("gatt_server exited with error: {:?}", e);
                        }
                    }
                }
            }
        }
    }
}

pub struct TemperatureMonitor {
    sd: &'static Softdevice,
    ticker: Ticker,
    connections: Vec<Connection, 2>,
}

impl TemperatureMonitor {
    pub fn new(sd: &'static Softdevice) -> Self {
        Self {
            sd,
            ticker: Ticker::every(Duration::from_secs(10)),
            connections: Vec::new(),
        }
    }

    fn handle_event(&mut self, conn: &Connection, event: &TemperatureServiceEvent) {
        match event {
            TemperatureServiceEvent::TemperatureNotificationsEnabled => {
                self.connections.push(conn.clone()).ok().unwrap();
                defmt::info!("notifications enabled!");
            }
            TemperatureServiceEvent::TemperatureNotificationsDisabled => {
                for i in 0..self.connections.len() {
                    if self.connections[i].handle() == conn.handle() {
                        self.connections.swap_remove(i);
                        break;
                    }
                }
                defmt::info!("notifications disabled!");
            }
            TemperatureServiceEvent::PeriodWrite(period) => {
                defmt::info!("adjust period!");
                self.ticker = Ticker::every(Duration::from_millis(*period as u64));
            }
        }
    }
}

pub struct TemperatureMonitorEvent(Connection, TemperatureServiceEvent);

impl Actor for TemperatureMonitor {
    type Configuration = &'static TemperatureService;
    type Message<'m> = TemperatureMonitorEvent;

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where Self: 'm, M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        service: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            loop {
                let inbox_fut = inbox.next();
                let ticker_fut = self.ticker.next();

                pin_mut!(inbox_fut);
                pin_mut!(ticker_fut);

                match select(inbox_fut, ticker_fut).await {
                    Either::Left((r, _)) => {
                        if let Some(mut m) = r {
                            let TemperatureMonitorEvent(conn, event) = m.message();
                            self.handle_event(conn, event);
                        }
                    }
                    Either::Right((_, _)) => {
                        let value: i8 = temperature_celsius(self.sd).unwrap().to_num();
                        defmt::trace!("Measuring temperature: {}", value);

                        service.temperature_set(value).unwrap();
                        for c in self.connections.iter() {
                            service.temperature_notify(&c, value).unwrap();
                        }
                    }
                }
            }
        }
    }
}

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
        inbox: &'m mut M,
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
        defmt::info!("advertising started!");

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

                defmt::info!("connection established: {}", conn.handle());

                acceptor.accept(conn).ok().unwrap();
            }
        }
    }
}

impl Acceptor for Address<'static, GattServer> {
    type Error = ();
    fn accept(&mut self, connection: Connection) -> Result<(), Self::Error> {
        self.notify(GattServerEvent::NewConnection(connection))
            .map_err(|_| ())
    }
}
