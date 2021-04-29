use core::future::Future;
use core::pin::Pin;
use drogue_device::{traits::lora::*, *};

pub struct LoraActor<D>
where
    D: LoraDriver,
{
    lora: D,
}

impl<D> LoraActor<D>
where
    D: LoraDriver,
{
    pub fn new(lora: D) -> Self {
        Self { lora }
    }
}

impl<D> Unpin for LoraActor<D> where D: LoraDriver {}

pub enum LoraCommand<'m> {
    Configure(&'m LoraConfig),
    Join,
    SendRecv(&'m [u8], &'m mut [u8]),
}

#[derive(Clone, Copy)]
pub enum LoraResult {
    Ok,
    Err(LoraError),
    OkSent(usize),
}

impl<D> Actor for LoraActor<D>
where
    D: LoraDriver,
{
    #[rustfmt::skip]
    type MaxNotifyQueueSize<'m> where Self: 'm = consts::U0;
    #[rustfmt::skip]
    type Configuration = ();
    #[rustfmt::skip]
    type Message<'m> where D: 'm = LoraCommand<'m>;
    #[rustfmt::skip]
    type Response<'m> where D: 'm = LoraResult;
    #[rustfmt::skip]
    type OnStartFuture<'m> where D: 'm = impl Future<Output = ()> + 'm;
    #[rustfmt::skip]
    type OnMessageFuture<'m> where D: 'm = impl Future<Output = Self::Response<'m>> + 'm;

    fn on_start<'m>(self: Pin<&'m mut Self>) -> Self::OnStartFuture<'m> {
        async move {}
    }

    fn on_message<'m>(
        mut self: Pin<&'m mut Self>,
        message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        async move {
            log_stack!();
            match message {
                LoraCommand::Configure(config) => {
                    if let Err(e) = self.lora.configure(&config).await {
                        log::error!("Error configuring: {:?}", e);
                        LoraResult::Err(e)
                    } else {
                        log::info!("LoRa driver configured");
                        LoraResult::Ok
                    }
                }
                LoraCommand::Join => {
                    if let Err(e) = self.lora.join(ConnectMode::OTAA).await {
                        log::error!("Error joining network: {:?}", e);
                        LoraResult::Err(e)
                    } else {
                        log::info!("Network joined");
                        LoraResult::Ok
                    }
                }
                LoraCommand::SendRecv(tx, rx) => {
                    match self.lora.send_recv(QoS::Confirmed, 1, tx, rx).await {
                        Err(e) => {
                            log::error!("Error sending message: {:?}", e);
                            LoraResult::Err(e)
                        }
                        Ok(len) => LoraResult::OkSent(len),
                    }
                }
            }
        }
    }
}
