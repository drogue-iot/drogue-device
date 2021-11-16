use crate::statistics::*;
use core::future::Future;
use drogue_device::{traits::led::TextDisplay, Actor, Address, Inbox};
use embassy::traits::uart::{Read, Write};

use crate::AppMatrix;

pub struct EchoServer<'a, U: Write + Read + 'a> {
    uart: U,
    _data: core::marker::PhantomData<&'a U>,
}

impl<'a, U: Write + Read + 'a> Unpin for EchoServer<'a, U> {}

impl<'a, U: Write + Read + 'a> EchoServer<'a, U> {
    pub fn new(uart: U) -> Self {
        Self {
            uart,
            _data: core::marker::PhantomData,
        }
    }
}

impl<'a, U: Write + Read + 'a> Actor for EchoServer<'a, U> {
    type Configuration = (Address<'a, AppMatrix>, Address<'a, Statistics>);
    type Message<'m>
    where
        'a: 'm,
    = ();

    type OnMountFuture<'m, M>
    where
        M: 'm,
        'a: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        _: Address<'static, Self>,
        _: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        let mut matrix = config.0;
        let statistics = config.1;
        async move {
            matrix.scroll("Hello, World!").await.unwrap();
            let mut buf = [0; 128];
            let motd = "Welcome to the Drogue Echo Service\r\n".as_bytes();
            buf[..motd.len()].clone_from_slice(motd);
            let _ = self.uart.write(&buf[..motd.len()]).await;

            defmt::info!("Application ready. Connect to the serial port to use the service.");
            loop {
                let _ = self.uart.read(&mut buf[..1]).await;
                let _ = self.uart.write(&buf[..1]).await;
                matrix.putc(buf[0] as char).unwrap();
                statistics
                    .request(StatisticsCommand::IncrementCharacterCount)
                    .unwrap()
                    .await;
            }
        }
    }
}
