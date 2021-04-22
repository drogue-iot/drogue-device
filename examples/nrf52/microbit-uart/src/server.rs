use crate::statistics::*;
use core::future::Future;
use core::pin::Pin;
use drogue_device::{
    traits::uart::{Read, Write},
    Actor, Address,
};

pub struct EchoServer<'a, U: Write + Read + 'a> {
    uart: U,
    statistics: Option<Address<'a, Statistics>>,
}

impl<'a, U: Write + Read + 'a> Unpin for EchoServer<'a, U> {}

impl<'a, U: Write + Read + 'a> EchoServer<'a, U> {
    pub fn new(uart: U) -> Self {
        Self {
            uart,
            statistics: None,
        }
    }
}

impl<'a, U: Write + Read + 'a> Actor for EchoServer<'a, U> {
    type Configuration = Address<'a, Statistics>;
    type Message<'m>
    where
        'a: 'm,
    = ();
    #[rustfmt::skip]
    type OnStartFuture<'m> where 'a: 'm = impl Future<Output = ()> + 'm;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where 'a: 'm = impl Future<Output = ()> + 'm;

    fn on_mount(&mut self, config: Self::Configuration) {
        self.statistics.replace(config);
    }

    fn on_start(mut self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
        let mut buf: [u8; 1] = [0; 1];
        async move {
            defmt::info!("Echo server started!");
            loop {
                let _ = self.uart.read(&mut buf).await;
                let _ = self.uart.write(&buf).await;
                if let Some(statistics) = self.statistics {
                    statistics
                        .process(&mut StatisticsCommand::IncrementCharacterCount)
                        .await;
                }
            }
        }
    }

    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        _: &'m mut Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {}
    }
}
