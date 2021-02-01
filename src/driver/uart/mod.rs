use crate::actor::Actor;
use crate::hal::uart::Uart as HalUart;
use crate::handler::{RequestHandler, Response};
use crate::interrupt::Interrupt;

pub struct Uart<T>
where
    T: HalUart,
{
    uart: T,
    tx: Option<UartTx>,
}

impl<T> Uart<T>
where
    T: HalUart,
{
    pub fn new(uart: T) -> Self {
        Self { uart, tx: None }
    }
}

impl<T> Actor for Uart<T> where T: HalUart {}

pub struct UartTx(pub &'static [u8]);

impl<T> RequestHandler<UartTx> for Uart<T>
where
    T: HalUart,
{
    type Response = ();

    fn on_request(&'static mut self, message: UartTx) -> Response<Self::Response> {
        if self.tx.is_none() {
            // self.uart.start_write(message.0);
            Response::defer(async move {})
        } else {
            Response::defer(async move {})
        }
    }
}

impl<T> Interrupt for Uart<T>
where
    T: HalUart,
{
    fn on_interrupt(&mut self) {
        //     self.uart.clear_update_interrupt_flag();
    }
}
