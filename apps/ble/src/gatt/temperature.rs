use core::future::Future;
use drogue_device::{Actor, Address, Inbox};
use nrf_softdevice::ble::Connection;
use nrf_softdevice::{temperature_celsius, Softdevice};

use embassy::time::Duration;
use heapless::Vec;

use embassy::time::Ticker;
use futures::{future::select, future::Either, pin_mut, StreamExt};

#[nrf_softdevice::gatt_service(uuid = "e95d6100-251d-470a-a062-fa1922dfa9a8")]
pub struct TemperatureService {
    #[characteristic(uuid = "e95d9250-251d-470a-a062-fa1922dfa9a8", read, notify)]
    temperature: i8,
    #[characteristic(uuid = "e95d1b25-251d-470a-a062-fa1922dfa9a8", read, write)]
    period: u16,
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
                info!("notifications enabled!");
            }
            TemperatureServiceEvent::TemperatureNotificationsDisabled => {
                for i in 0..self.connections.len() {
                    if self.connections[i].handle() == conn.handle() {
                        self.connections.swap_remove(i);
                        break;
                    }
                }
                info!("notifications disabled!");
            }
            TemperatureServiceEvent::PeriodWrite(period) => {
                info!("Adjusting measurement interval to {} milliseconds!", period);
                self.ticker = Ticker::every(Duration::from_millis(*period as u64));
            }
        }
    }
}

impl Actor for TemperatureMonitor {
    type Configuration = &'static TemperatureService;
    type Message<'m> = (Connection, TemperatureServiceEvent);

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
                    Either::Left((mut m, _)) => {
                        let (conn, event) = m.message();
                        self.handle_event(conn, event);
                    }
                    Either::Right((_, _)) => {
                        let value: i8 = temperature_celsius(self.sd).unwrap().to_num();
                        trace!("Measured temperature: {}℃", value);

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
