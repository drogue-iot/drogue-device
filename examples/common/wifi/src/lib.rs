#![no_std]
#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(min_type_alias_impl_trait)]
#![feature(impl_trait_in_bindings)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use core::future::Future;
use core::pin::Pin;
use drogue_device::{
    actors::{
        button::{ButtonEvent, FromButtonEvent},
        wifi::{Adapter, Socket, WifiAdapter},
    },
    traits::{ip::*, wifi::*},
    Actor, Address,
};
pub enum Command {
    Send,
}

impl<A: Adapter> FromButtonEvent<Command> for App<A> {
    fn from(event: ButtonEvent) -> Option<Command> {
        match event {
            ButtonEvent::Pressed => None,
            ButtonEvent::Released => Some(Command::Send),
        }
    }
}

pub struct App<A: Adapter + 'static> {
    ssid: &'static str,
    psk: &'static str,
    ip: IpAddress,
    port: u16,
    adapter: Option<WifiAdapter<'static, A>>,
    socket: Option<Socket<'static, A>>,
}

impl<A: Adapter> App<A> {
    pub fn new(ssid: &'static str, psk: &'static str, ip: IpAddress, port: u16) -> Self {
        Self {
            ssid,
            psk,
            ip,
            port,
            adapter: None,
            socket: None,
        }
    }
}

impl<A: Adapter> Actor for App<A> {
    type Configuration = WifiAdapter<'static, A>;
    #[rustfmt::skip]
    type Message<'m> where A: 'm = Command;
    #[rustfmt::skip]
    type OnStartFuture<'m> where A: 'm = impl Future<Output = ()> + 'm;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where A: 'm = impl Future<Output = ()> + 'm;

    fn on_mount(&mut self, _: Address<'static, Self>, config: Self::Configuration) {
        self.adapter.replace(config);
    }

    fn on_start<'m>(mut self: Pin<&'m mut Self>) -> Self::OnStartFuture<'m> {
        async move {
            let adapter = self.adapter.take().unwrap();
            log::info!("Joining access point");
            adapter
                .join(Join::Wpa {
                    ssid: self.ssid,
                    password: self.psk,
                })
                .await
                .expect("Error joining wifi");
            log::info!("Joined access point");

            let socket = adapter.socket().await;

            log::info!("Connecting to {}:{}", self.ip, self.port);
            let result = socket
                .connect(IpProtocol::Tcp, SocketAddress::new(self.ip, self.port))
                .await;
            match result {
                Ok(_) => {
                    self.adapter.replace(adapter);
                    self.socket.replace(socket);
                    log::info!("Connected to {:?}!", self.ip);
                }
                Err(e) => {
                    log::warn!("Error connecting: {:?}", e);
                }
            }
        }
    }

    fn on_message<'m>(
        mut self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            match message {
                Command::Send => {
                    log::info!("Pinging server..");

                    let socket = self.socket.take().expect("socket not bound!");
                    let result = socket.send(b"PING").await;
                    match result {
                        Ok(_) => {
                            log::debug!("Data sent");
                            let mut rx_buf = [0; 8];
                            loop {
                                let result = socket.recv(&mut rx_buf[..]).await;
                                match result {
                                    Ok(len) if &rx_buf[0..len] == b"PING" => {
                                        log::info!("Ping response received");
                                        break;
                                    }
                                    Ok(len) => {
                                        log::warn!(
                                            "Unexpected response of {} bytes: {:?}",
                                            len,
                                            &rx_buf[0..len]
                                        );
                                        break;
                                    }
                                    Err(e) => {
                                        log::warn!("Error reading response: {:?}", e);
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            log::warn!("Error pinging server: {:?}", e);
                        }
                    }
                    self.socket.replace(socket);
                }
            }
        }
    }
}
