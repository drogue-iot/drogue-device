use crate::prelude::*;

pub enum SpiError {
    Overrun,
    ModeFault,
    Crc,
    Unknown,
}

pub trait SpiBus: Actor {
    type Word;
    fn transfer(self, transfer: SpiTransfer<Self::Word>) -> Response<Self, Result<(), SpiError>>;
}

impl<'b, B> RequestHandler<SpiTransfer<'b, B::Word>> for B
where
    B: SpiBus + 'static,
{
    type Response = Result<(), SpiError>;

    fn on_request(self, message: SpiTransfer<'b, B::Word>) -> Response<Self, Self::Response> {
        log::info!("[spi-bus] on_request");
        self.transfer(message)
    }
}

pub struct SpiTransfer<'b, W>(pub &'b mut [W]);

/*
impl<B: SpiBus> Address<B> {
    pub async fn spi_transfer<'b>(&self, buffer: &mut [B::Word]) -> Result<(), SpiError>
    where
        Self: RequestHandler<SpiTransfer<'b, B::Word>>,
    {
        self.request_panicking(SpiTransfer(buffer)).await
    }
}
 */
