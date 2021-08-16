#![no_std]
#![macro_use]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
#![feature(concat_idents)]

use core::future::Future;
use core::pin::Pin;
use drogue_device::{
    actors::button::{ButtonEvent, FromButtonEvent},
    clients::http::*,
    traits::{ip::*, tcp::*},
    Actor, Address,
};
pub enum Command {
    Send,
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
    type OnStartFuture<'m> where S: 'm = impl Future<Output = ()> + 'm;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where S: 'm = impl Future<Output = ()> + 'm;

    fn on_mount(&mut self, _: Address<'static, Self>, config: Self::Configuration) {
        self.socket.replace(config);
    }

    fn on_start<'m>(self: Pin<&'m mut Self>) -> Self::OnStartFuture<'m> {
        async move {}
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            match message {
                Command::Send => {
                    log::info!("Sending data");
                    let this = unsafe { self.get_unchecked_mut() };
                    let socket = this.socket.as_mut().unwrap();
                    let mut client =
                        HttpClient::new(socket, this.ip, this.port, this.username, this.password);

                    let mut rx_buf = [0; 1024];
                    let response_len = client
                        .post(
                            "/v1/foo",
                            b"Hello from Drogue",
                            "application/plain",
                            &mut rx_buf[..],
                        )
                        .await;
                    if let Ok(response_len) = response_len {
                        log::info!(
                            "Response: {}",
                            core::str::from_utf8(&rx_buf[..response_len]).unwrap()
                        );
                    }
                }
            }
        }
    }
}
