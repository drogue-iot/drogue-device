use core::future::Future;
use crate::{Actor, Address, Inbox};
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
    service: &'static TemperatureService,
}

impl TemperatureMonitor {
    pub fn new(sd: &'static Softdevice, service: &'static TemperatureService) -> Self {
        Self {
            sd,
            ticker: Ticker::every(Duration::from_secs(10)),
            connections: Vec::new(),
            service,
        }
    }

    fn handle_event(&mut self, conn: &Connection, event: &TemperatureServiceEvent) {
        match event {
            TemperatureServiceEvent::TemperatureCccdWrite { notifications } => {
                if *notifications {
                    self.connections.push(conn.clone()).ok().unwrap();
                    info!("notifications enabled!");
                } else {
                    for i in 0..self.connections.len() {
                        if self.connections[i].handle() == conn.handle() {
                            self.connections.swap_remove(i);
                            break;
                        }
                    }
                    info!("notifications disabled!");
                }
            }
            TemperatureServiceEvent::PeriodWrite(period) => {
                info!("Adjusting measurement interval to {} milliseconds!", period);
                self.ticker = Ticker::every(Duration::from_millis(*period as u64));
            }
        }
    }
}

impl Actor for TemperatureMonitor {
    type Message<'m> = (Connection, TemperatureServiceEvent);

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
                let inbox_fut = inbox.next();
                let ticker_fut = self.ticker.next();

                pin_mut!(inbox_fut);
                pin_mut!(ticker_fut);

                match select(inbox_fut, ticker_fut).await {
                    Either::Left((r, _)) => {
                        if let Some(mut m) = r {
                            let (conn, event) = m.message();
                            self.handle_event(conn, event);
                        }
                    }
                    Either::Right((_, _)) => {
                        let value: i8 = temperature_celsius(self.sd).unwrap().to_num();
                        trace!("Measured temperature: {}℃", value);

                        self.service.temperature_set(value).unwrap();
                        for c in self.connections.iter() {
                            self.service.temperature_notify(&c, value).unwrap();
                        }
                    }
                }
            }
        }
    }
}
