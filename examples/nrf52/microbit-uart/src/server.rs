use core::future::Future;
use core::pin::Pin;
use drogue_device::{
    traits::uart::{Read, Write},
    Actor,
};

pub struct EchoServer<U: Write + Read + 'static> {
    uart: U,
}

impl<'a, U: Write + Read + 'static> Unpin for EchoServer<U> {}

impl<'a, U: Write + Read + 'static> EchoServer<U> {
    pub fn new(uart: U) -> Self {
        Self { uart }
    }
}

impl<U: Write + Read + 'static> Actor for EchoServer<U> {
    type Configuration = ();
    type Message<'a> = ();
    type OnStartFuture<'a> = impl Future<Output = ()> + 'a;
    type OnMessageFuture<'a> = impl Future<Output = ()> + 'a;

    fn on_start(mut self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
        let mut buf: [u8; 1] = [0; 1];
        async move {
            defmt::info!("Echo server started!");
            loop {
                let _ = self.uart.read(&mut buf).await;
                let _ = self.uart.write(&buf).await;
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
