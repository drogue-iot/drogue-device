use crate::statistics::*;
use core::future::Future;
use drogue_device::{traits::led::TextDisplay, Actor, Address, Inbox};
use embedded_hal_async::serial::{Read, Write};

use crate::AppMatrix;

pub struct EchoServer<U: Write + Read + 'static> {
    uart: U,
    matrix: Address<AppMatrix>,
    statistics: Address<Statistics>,
}

impl<U: Write + Read + 'static> EchoServer<U> {
    pub fn new(uart: U, matrix: Address<AppMatrix>, statistics: Address<Statistics>) -> Self {
        Self {
            uart,
            matrix,
            statistics,
        }
    }
}

impl<U: Write + Read + 'static> Actor for EchoServer<U> {
    type OnMountFuture<'m, M>
    where
        M: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(&'m mut self, _: Address<Self>, _: &'m mut M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            self.matrix.scroll("Hello, World!").await.unwrap();
            let mut buf = [0; 128];
            let motd = "Welcome to the Drogue Echo Service\r\n".as_bytes();
            buf[..motd.len()].clone_from_slice(motd);
            let _ = self.uart.write(&buf[..motd.len()]).await;

            defmt::info!("Application ready. Connect to the serial port to use the service.");
            loop {
                let _ = self.uart.read(&mut buf[..1]).await;
                let _ = self.uart.write(&buf[..1]).await;
                self.matrix.putc(buf[0] as char).unwrap();
                self.statistics
                    .request(StatisticsCommand::IncrementCharacterCount)
                    .unwrap()
                    .await;
            }
        }
    }
}
