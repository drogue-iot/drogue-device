#![no_std]
#![macro_use]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]

pub(crate) mod fmt;

use core::fmt::Write;
use core::future::Future;
use drogue_device::{
    actors::button::{ButtonEvent, FromButtonEvent},
    actors::sensors::SensorMonitor,
    clients::http::*,
    domain::{temperature::Celsius, SensorAcquisition},
    traits::{ip::*, tcp::*},
    Actor, Address, Inbox,
};
use heapless::String;

pub enum Command {
    Send,
    Update(SensorAcquisition<Celsius>),
}

impl<S> FromButtonEvent<Command> for App<S>
where
    S: TcpSocket + 'static,
{
    fn from(event: ButtonEvent) -> Option<Command> {
        match event {
            ButtonEvent::Pressed => None,
            ButtonEvent::Released => Some(Command::Send),
        }
    }
}

pub struct App<S>
where
    S: TcpSocket + 'static,
{
    ip: IpAddress,
    port: u16,
    username: &'static str,
    password: &'static str,
    socket: Option<S>,
}

impl<S> App<S>
where
    S: TcpSocket + 'static,
{
    pub fn new(ip: IpAddress, port: u16, username: &'static str, password: &'static str) -> Self {
        Self {
            ip,
            port,
            username,
            password,
            socket: None,
        }
    }
}

impl<S> Actor for App<S>
where
    S: TcpSocket + 'static,
{
    type Configuration = S;
    #[rustfmt::skip]
    type Message<'m> where S: 'm = Command;

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where S: 'm, M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        self.socket.replace(config);
        async move {
            let mut temperature: Option<SensorAcquisition<Celsius>> = None;
            loop {
                match inbox.next().await {
                    Some(mut m) => match m.message() {
                        Command::Update(t) => {
                            info!("Updating current app temperature measurement: {}", t);
                            temperature.replace(t.clone());
                        }
                        Command::Send => match &temperature {
                            Some(t) => {
                                info!("Sending temperature measurement");
                                let socket = self.socket.as_mut().unwrap();
                                let mut client = HttpClient::new(
                                    socket,
                                    self.ip,
                                    self.port,
                                    self.username,
                                    self.password,
                                );

                                let mut tx: String<64> = String::new();
                                write!(
                                    tx,
                                    "{{\"temp\": {}, \"humidity\": {}}}",
                                    t.temperature, t.relative_humidity
                                )
                                .unwrap();
                                let mut rx_buf = [0; 1024];
                                let response_len = client
                                    .post(
                                        "/v1/foo",
                                        tx.as_bytes(),
                                        "application/json",
                                        &mut rx_buf[..],
                                    )
                                    .await;
                                if let Ok(response_len) = response_len {
                                    info!(
                                        "Response: {}",
                                        core::str::from_utf8(&rx_buf[..response_len]).unwrap()
                                    );
                                }
                            }
                            None => {
                                info!("No temperature value found, not sending measurement");
                            }
                        },
                    },
                    _ => {}
                }
            }
        }
    }
}

pub struct AppAddress<S: TcpSocket + 'static> {
    address: Address<'static, App<S>>,
}

impl<S: TcpSocket> From<Address<'static, App<S>>> for AppAddress<S> {
    fn from(address: Address<'static, App<S>>) -> Self {
        Self { address }
    }
}

impl<S: TcpSocket> SensorMonitor<Celsius> for AppAddress<S> {
    fn notify(&self, value: SensorAcquisition<Celsius>) {
        // Ignore channel full error
        let _ = self.address.notify(Command::Update(value));
    }
}
