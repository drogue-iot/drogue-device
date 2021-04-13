use core::future::Future;
use core::pin::Pin;
use drogue_device_kernel::Actor;
use embassy::traits::uart::{Read, Write};
use futures::pin_mut;

pub struct EchoServer<U: Write + Read + 'static> {
    uart: Option<U>,
}

impl<U: Write + Read> Unpin for EchoServer<U> {}

impl<U: Write + Read> EchoServer<U> {
    pub fn new(uart: U) -> Self {
        Self { uart: Some(uart) }
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
            let uart = self.uart.take().unwrap();
            pin_mut!(uart);
            defmt::info!("Echo server started!");
            loop {
                let _ = uart.as_mut().read(&mut buf).await;
                let _ = uart.as_mut().write(&buf).await;
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
