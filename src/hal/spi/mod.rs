use crate::domain::time::duration::Milliseconds;
use crate::hal::arbitrator::BusTransaction;
use crate::hal::delayer::Delayer;
use crate::prelude::*;
use core::cell::RefCell;
use embedded_hal::digital::v2::OutputPin;

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
        self.transfer(message)
    }
}

pub struct SpiTransfer<'b, W>(pub &'b mut [W]);

impl<SPI> BusTransaction<SPI>
where
    SPI: SpiBus,
{
    pub async fn spi_transfer<'b>(&self, buffer: &mut [SPI::Word]) -> Result<(), SpiError> {
        self.bus.request_panicking(SpiTransfer(buffer)).await
    }
}

pub struct ChipSelect<PIN, D>
where
    PIN: OutputPin + 'static,
    D: Delayer + 'static,
{
    select_delay: Milliseconds,
    pin: RefCell<PIN>,
    delayer: Option<Address<D>>,
}

impl<PIN, D> ChipSelect<PIN, D>
where
    PIN: OutputPin,
    D: Delayer,
{
    pub fn new(pin: PIN, select_delay: Milliseconds) -> Self {
        Self {
            select_delay,
            pin: RefCell::new(pin),
            delayer: None,
        }
    }

    pub(crate) fn set_delayer(&mut self, delayer: Address<D>) {
        self.delayer.replace(delayer);
    }

    pub async fn select(&self) -> Selected<'_, PIN, D> {
        self.pin.borrow_mut().set_low();
        self.delayer.unwrap().delay(self.select_delay).await;
        Selected::new(&self)
    }

    fn deselect(&self) {
        self.pin.borrow_mut().set_high();
    }
}

pub struct Selected<'cs, PIN, D>
where
    PIN: OutputPin + 'static,
    D: Delayer + 'static,
{
    cs: &'cs ChipSelect<PIN, D>,
}

impl<'cs, PIN, D> Selected<'cs, PIN, D>
where
    PIN: OutputPin + 'static,
    D: Delayer + 'static,
{
    fn new(cs: &'cs ChipSelect<PIN, D>) -> Self {
        Self { cs }
    }
}

impl<'cs, PIN, D> Drop for Selected<'cs, PIN, D>
where
    PIN: OutputPin + 'static,
    D: Delayer + 'static,
{
    fn drop(&mut self) {
        self.cs.deselect();
    }
}
