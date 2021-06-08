use crate::{
    kernel::actor::{Actor, Address},
    traits::lora::*,
};
use core::{future::Future, pin::Pin};

/// Messages handled by lora actor
pub enum LoraRequest<'m> {
    Configure(&'m LoraConfig),
    Join(ConnectMode),
    Send(QoS, Port, &'m [u8]),
    SendRecv(QoS, Port, &'m [u8], &'m mut [u8]),
}

impl<'a, D> LoraDriver for Address<'a, LoraActor<D>>
where
    D: LoraDriver + 'a,
{
    #[rustfmt::skip]
    type ConfigureFuture<'m> where 'a: 'm = impl Future<Output = Result<(), LoraError>> + 'm;
    fn configure<'m>(&'m mut self, config: &'m LoraConfig) -> Self::ConfigureFuture<'m> {
        async move {
            self.request(LoraRequest::Configure(config))
                .unwrap()
                .await
                .map(|_| ())
        }
    }

    #[rustfmt::skip]
    type JoinFuture<'m> where 'a: 'm = impl Future<Output = Result<(), LoraError>> + 'm;
    fn join<'m>(&'m mut self, mode: ConnectMode) -> Self::JoinFuture<'m> {
        async move {
            self.request(LoraRequest::Join(mode))
                .unwrap()
                .await
                .map(|_| ())
        }
    }

    #[rustfmt::skip]
    type SendFuture<'m> where 'a: 'm = impl Future<Output = Result<(), LoraError>> + 'm;
    fn send<'m>(&'m mut self, qos: QoS, port: Port, data: &'m [u8]) -> Self::SendFuture<'m> {
        async move {
            self.request(LoraRequest::Send(qos, port, data))
                .unwrap()
                .await
                .map(|_| ())
        }
    }

    #[rustfmt::skip]
    type SendRecvFuture<'m> where 'a: 'm = impl Future<Output = Result<usize, LoraError>> + 'm;
    fn send_recv<'m>(
        &'m mut self,
        qos: QoS,
        port: Port,
        data: &'m [u8],
        rx: &'m mut [u8],
    ) -> Self::SendRecvFuture<'m> {
        async move {
            self.request(LoraRequest::SendRecv(qos, port, data, rx))
                .unwrap()
                .await
        }
    }
}

pub struct LoraActor<D>
where
    D: LoraDriver + 'static,
{
    driver: D,
}

impl<D> LoraActor<D>
where
    D: LoraDriver + 'static,
{
    pub fn new(driver: D) -> Self {
        Self { driver }
    }
}

impl<D> Actor for LoraActor<D>
where
    D: LoraDriver + 'static,
{
    type Configuration = ();

    #[rustfmt::skip]
    type Message<'m> where D: 'm = LoraRequest<'m>;
    type Response = Result<usize, LoraError>;

    #[rustfmt::skip]
    type OnStartFuture<'m> where D: 'm = impl Future<Output = ()> + 'm;
    fn on_start(self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
        async move {}
    }

    #[rustfmt::skip]
    type OnMessageFuture<'m> where D: 'm = impl Future<Output = Self::Response> + 'm;
    fn on_message<'m>(
        self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            let this = unsafe { self.get_unchecked_mut() };
            let driver = &mut this.driver;
            match message {
                LoraRequest::Configure(config) => driver.configure(config).await.map(|_| 0),
                LoraRequest::Join(mode) => driver.join(mode).await.map(|_| 0),
                LoraRequest::Send(qos, port, buf) => driver.send(qos, port, buf).await.map(|_| 0),
                LoraRequest::SendRecv(qos, port, buf, rx) => {
                    driver.send_recv(qos, port, buf, rx).await
                }
            }
        }
    }
}
