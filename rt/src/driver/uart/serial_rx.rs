use crate::hal::uart::UartRx;
use crate::prelude::*;

use embedded_hal::serial::Read;

pub struct SerialRx<D, RX>
where
    D: Actor<Request = SerialData, Response = ()> + 'static,
    RX: Read<u8> + UartRx + 'static,
{
    rx: RX,
    handler: Option<Address<D>>,
}

impl<D, RX> SerialRx<D, RX>
where
    D: Actor<Request = SerialData, Response = ()> + 'static,
    RX: Read<u8> + UartRx + 'static,
{
    pub fn new(rx: RX) -> Self {
        Self { rx, handler: None }
    }
}

impl<D, RX> Actor for SerialRx<D, RX>
where
    D: Actor<Request = SerialData, Response = ()> + 'static,
    RX: Read<u8> + UartRx + 'static,
{
    type Configuration = Address<D>;
    type Request = ();
    type Response = ();
    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.handler.replace(config);
    }

    fn on_start(mut self) -> Completion<Self> {
        self.rx.enable_interrupt();
        Completion::immediate(self)
    }

    fn on_request(self, _: Self::Request) -> Response<Self> {
        Response::immediate(self, ())
    }
}

impl<D, RX> Interrupt for SerialRx<D, RX>
where
    D: Actor<Request = SerialData, Response = ()> + 'static,
    RX: Read<u8> + UartRx + 'static,
{
    fn on_interrupt(&mut self) {
        if self.rx.check_interrupt() {
            let handler = self.handler.as_ref().unwrap();
            loop {
                match self.rx.read() {
                    Ok(b) => {
                        handler.notify(SerialData(b));
                    }
                    Err(nb::Error::WouldBlock) => {
                        break;
                    }
                    Err(e) => {
                        log::warn!("Error while reading");
                        break;
                    }
                }
            }
        }
        self.rx.clear_interrupt();
    }
}

#[derive(Clone)]
pub struct SerialData(pub u8);
