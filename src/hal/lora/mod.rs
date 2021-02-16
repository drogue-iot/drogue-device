use crate::actor::Actor;
use crate::address::Address;
use crate::handler::{RequestHandler, Response};
pub use drogue_lora::*;

/// API for accessing LoRa modules
impl<A> Address<A>
where
    A: LoraDriver,
{
    pub async fn initialize(&self) -> Result<(), LoraError> {
        self.request(Initialize).await
    }

    pub async fn configure<'a>(&self, config: &'a LoraConfig) -> Result<(), LoraError> {
        self.request_panicking(Configure(config)).await
    }

    pub async fn reset(&self, mode: ResetMode) -> Result<(), LoraError> {
        self.request(Reset(mode)).await
    }

    pub async fn join(&self, mode: ConnectMode) -> Result<(), LoraError> {
        self.request(Join(mode)).await
    }

    pub async fn send<'a>(&self, qos: QoS, port: Port, data: &'a [u8]) -> Result<(), LoraError> {
        self.request_panicking(Send(qos, port, data)).await
    }
}

#[derive(Debug)]
pub enum LoraError {
    SendError,
    RecvError,
    RecvTimeout,
    NotInitialized,
    OtherError,
}

/// Trait for a LoRa driver.
pub trait LoraDriver: Actor {
    /// Initialize the LoRa module. This should perform any reset of the peripheral and make sure it is ready to operate.
    fn initialize(self, message: Initialize) -> Response<Self, Result<(), LoraError>>;

    /// Configure the LoRa module.
    fn configure<'a>(self, message: Configure<'a>) -> Response<Self, Result<(), LoraError>>;

    /// Perform a reset of the LoRa module, retaining configuration previously applied.
    fn reset(self, message: Reset) -> Response<Self, Result<(), LoraError>>;

    /// Join a LoRaWAN network using the specified connect mode.
    fn join(self, message: Join) -> Response<Self, Result<(), LoraError>>;

    /// Send telemetry data with a given Quality-of-Service on a specific port.
    fn send<'a>(self, message: Send<'a>) -> Response<Self, Result<(), LoraError>>;
}

/// Message types and handlers for the LoraDriver trait.

#[derive(Debug)]
pub struct Initialize;
#[derive(Debug)]
pub struct Configure<'a>(pub &'a LoraConfig);
#[derive(Debug)]
pub struct Join(pub ConnectMode);
#[derive(Debug)]
pub struct Reset(pub ResetMode);
#[derive(Debug)]
pub struct Send<'a>(pub QoS, pub Port, pub &'a [u8]);
#[derive(Debug)]
pub struct Recv<'a>(pub &'a mut [u8]);

impl<A> RequestHandler<Initialize> for A
where
    A: LoraDriver,
{
    type Response = Result<(), LoraError>;
    fn on_request(self, message: Initialize) -> Response<Self, Self::Response> {
        self.initialize(message)
    }
}

impl<'a, A> RequestHandler<Configure<'a>> for A
where
    A: LoraDriver,
{
    type Response = Result<(), LoraError>;
    fn on_request(self, message: Configure<'a>) -> Response<Self, Self::Response> {
        self.configure(message)
    }
}

impl<A> RequestHandler<Reset> for A
where
    A: LoraDriver,
{
    type Response = Result<(), LoraError>;
    fn on_request(self, message: Reset) -> Response<Self, Self::Response> {
        self.reset(message)
    }
}

impl<A> RequestHandler<Join> for A
where
    A: LoraDriver,
{
    type Response = Result<(), LoraError>;
    fn on_request(self, message: Join) -> Response<Self, Self::Response> {
        self.join(message)
    }
}

impl<'a, A> RequestHandler<Send<'a>> for A
where
    A: LoraDriver,
{
    type Response = Result<(), LoraError>;
    fn on_request(self, message: Send<'a>) -> Response<Self, Self::Response> {
        self.send(message)
    }
}
