use crate::actor::Actor;
use crate::address::Address;
use crate::bind::Bind;
use crate::driver::lora::*;
use crate::driver::uart::UartPeripheral as Uart;
use crate::hal::uart::Uart as HalUart;
use crate::handler::{Completion, NotifyHandler, RequestHandler, Response};

pub struct Rak811<U>
where
    U: HalUart + 'static,
{
    uart: Option<Address<Uart<U>>>,
}

impl<U> Rak811<U>
where
    U: HalUart,
{
    pub fn new() -> Self {
        Self { uart: None }
    }
}

impl<U> Bind<Uart<U>> for Rak811<U>
where
    U: HalUart,
{
    fn on_bind(&mut self, address: Address<Uart<U>>) {
        self.uart.replace(address);
    }
}

impl<U> Actor for Rak811<U> where U: HalUart {}

impl<U> Configurable for Rak811<U>
where
    U: HalUart,
{
    type Configuration = Address<Uart<U>>;
    fn configure(&mut self, config: Self::Configuration) {
        self.uart.replace(config);
    }
}

impl<U> NotifyHandler<Reset> for Rak811<U>
where
    U: HalUart,
{
    fn on_notify(self, message: Reset) -> Completion<Self> {
        Completion::immediate(self)
    }
}

impl<'a, U> RequestHandler<Configure<'a>> for Rak811<U>
where
    U: HalUart,
{
    type Response = Result<(), DriverError>;
    fn on_request(self, message: Configure<'a>) -> Response<Self, Self::Response> {
        Response::immediate(self, Ok(()))
    }
}

impl<U> RequestHandler<Join> for Rak811<U>
where
    U: HalUart,
{
    type Response = Result<(), DriverError>;
    fn on_request(self, message: Join) -> Response<Self, Self::Response> {
        Response::immediate(self, Ok(()))
    }
}

impl<'a, U> RequestHandler<Send<'a>> for Rak811<U>
where
    U: HalUart,
{
    type Response = Result<(), DriverError>;
    fn on_request(self, message: Send<'a>) -> Response<Self, Self::Response> {
        Response::immediate(self, Ok(()))
    }
}
