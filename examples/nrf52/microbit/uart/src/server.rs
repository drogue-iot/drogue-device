use crate::statistics::*;
use core::future::Future;
use drogue_device::{traits::led::TextDisplay, Actor, Address, Inbox};
use embassy::time::Duration;
use embedded_hal_async::serial::{Read, Write};

use crate::AppMatrix;

pub struct EchoServer<U: Write + Read + 'static> {
    uart: U,
    matrix: AppMatrix,
    statistics: Address<StatisticsCommand>,
}

impl<U: Write + Read + 'static> EchoServer<U> {
    pub fn new(uart: U, matrix: AppMatrix, statistics: Address<StatisticsCommand>) -> Self {
        Self {
            uart,
            matrix,
            statistics,
        }
    }
}

impl<U: Write + Read + 'static> Actor for EchoServer<U> {
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        M: 'm + Inbox<Self::Message<'m>>;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self::Message<'m>>,
        _: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        async move {
            self.matrix.scroll("Hello, World!").await;
            let mut buf = [0; 128];
            let motd = "Welcome to the Drogue Echo Service\r\n".as_bytes();
            buf[..motd.len()].clone_from_slice(motd);
            let _ = self.uart.write(&buf[..motd.len()]).await;

            defmt::info!("Application ready. Connect to the serial port to use the service.");
            loop {
                let _ = self.uart.read(&mut buf[..1]).await;
                let _ = self.uart.write(&buf[..1]).await;
                let _ = TextDisplay::display(
                    &mut self.matrix,
                    buf[0] as char,
                    Duration::from_millis(500),
                )
                .await;
                self.statistics
                    .notify(StatisticsCommand::IncrementCharacterCount)
                    .await;
            }
        }
    }
}
