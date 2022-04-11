use crate::{drivers::ble::gatt::temperature::*, Actor, Address, Inbox};
use core::future::Future;
use nrf_softdevice::ble::Connection;
use nrf_softdevice::{temperature_celsius, Softdevice};

use embassy::time::Duration;
use heapless::Vec;

use embassy::time::Ticker;
use futures::{future::select, future::Either, pin_mut, StreamExt};

pub struct TemperatureMonitor {
    sd: &'static Softdevice,
    ticker: Ticker,
    connections: Vec<Connection, 2>,
    service: &'static TemperatureService,
    notify: bool,
}

impl TemperatureMonitor {
    pub fn new(sd: &'static Softdevice, service: &'static TemperatureService) -> Self {
        Self {
            sd,
            ticker: Ticker::every(Duration::from_secs(10)),
            connections: Vec::new(),
            service,
            notify: false,
        }
    }

    fn add_connection(&mut self, conn: &Connection) {
        self.connections.push(conn.clone()).ok().unwrap();
    }

    fn remove_connection(&mut self, conn: &Connection) {
        for i in 0..self.connections.len() {
            if self.connections[i].handle() == conn.handle() {
                self.connections.swap_remove(i);
            }
        }
    }

    fn handle_event(&mut self, event: &TemperatureServiceEvent) {
        match event {
            TemperatureServiceEvent::TemperatureCccdWrite { notifications } => {
                self.notify = *notifications;
            }
            TemperatureServiceEvent::PeriodWrite(period) => {
                info!("Adjusting measurement interval to {} milliseconds!", period);
                self.ticker = Ticker::every(Duration::from_millis(*period as u64));
            }
        }
    }
}

pub enum MonitorEvent {
    AddConnection(Connection),
    RemoveConnection(Connection),
    Event(TemperatureServiceEvent),
}

impl Actor for TemperatureMonitor {
    type Message<'m> = MonitorEvent;
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<MonitorEvent>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<MonitorEvent>,
        mut inbox: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<MonitorEvent> + 'm,
    {
        async move {
            loop {
                let inbox_fut = inbox.next();
                let ticker_fut = self.ticker.next();

                pin_mut!(inbox_fut);
                pin_mut!(ticker_fut);

                match select(inbox_fut, ticker_fut).await {
                    Either::Left((m, _)) => match m {
                        MonitorEvent::AddConnection(conn) => {
                            self.add_connection(&conn);
                        }
                        MonitorEvent::RemoveConnection(conn) => {
                            self.remove_connection(&conn);
                        }
                        MonitorEvent::Event(event) => {
                            self.handle_event(&event);
                        }
                    },
                    Either::Right((_, _)) => {
                        let value: i8 = temperature_celsius(self.sd).unwrap().to_num();
                        trace!("Measured temperature: {}℃", value);

                        self.service.temperature_set(value).unwrap();
                        if self.notify {
                            for c in self.connections.iter() {
                                self.service.temperature_notify(&c, value).unwrap();
                            }
                        }
                    }
                }
            }
        }
    }
}
