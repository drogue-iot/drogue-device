use crate::{
    kernel::actor::{Actor, Address, Inbox},
    traits::lora::*,
};
use core::future::Future;

/// Messages handled by lora actor
pub enum LoraRequest<'m> {
    Join(JoinMode),
    Send(QoS, Port, &'m [u8]),
    SendRecv(QoS, Port, &'m [u8], &'m mut [u8]),
}

impl<D> LoraDriver for Address<LoraActor<D>>
where
    D: LoraDriver + 'static,
{
    type JoinFuture<'m> = impl Future<Output = Result<(), LoraError>> + 'm;
    fn join<'m>(&'m mut self, mode: JoinMode) -> Self::JoinFuture<'m> {
        async move {
            self.request(LoraRequest::Join(mode))
                .unwrap()
                .await
                .unwrap()
                .map(|_| ())
        }
    }

    type SendFuture<'m> = impl Future<Output = Result<(), LoraError>> + 'm;
    fn send<'m>(&'m mut self, qos: QoS, port: Port, data: &'m [u8]) -> Self::SendFuture<'m> {
        async move {
            self.request(LoraRequest::Send(qos, port, data))
                .unwrap()
                .await
                .unwrap()
                .map(|_| ())
        }
    }

    type SendRecvFuture<'m> = impl Future<Output = Result<usize, LoraError>> + 'm;
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
                .unwrap()
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
    type Message<'m> = LoraRequest<'m> where D: 'm;
    type Response = Option<Result<usize, LoraError>>;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm where D: 'm, M: 'm + Inbox<Self>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            let driver = &mut self.driver;
            loop {
                if let Some(mut m) = inbox.next().await {
                    let response = match m.message() {
                        LoraRequest::Join(mode) => driver.join(*mode).await.map(|_| 0),
                        LoraRequest::Send(qos, port, buf) => {
                            driver.send(*qos, *port, buf).await.map(|_| 0)
                        }
                        LoraRequest::SendRecv(qos, port, buf, rx) => {
                            driver.send_recv(*qos, *port, buf, rx).await
                        }
                    };
                    m.set_response(Some(response));
                }
            }
        }
    }
}
