use crate::prelude::*;
pub use drogue_lora::*;

/// API for accessing LoRa modules
#[allow(clippy::needless_lifetimes)]
impl<A> Address<A>
where
    A: LoraDriver,
{
    /// Configure the LoRa module with the provided config.
    pub async fn configure<'a>(&self, config: &'a LoraConfig) -> Result<(), LoraError> {
        self.request_panicking(Configure(config)).await
    }

    /// Reset the LoRa module.
    pub async fn reset(&self, mode: ResetMode) -> Result<(), LoraError> {
        self.request(Reset(mode)).await
    }

    /// Join a LoRaWAN network with the given connect mode.
    pub async fn join(&self, mode: ConnectMode) -> Result<(), LoraError> {
        self.request(Join(mode)).await
    }

    /// Send data on a specific port with a given quality of service.
    pub async fn send<'a>(&self, qos: QoS, port: Port, data: &'a [u8]) -> Result<(), LoraError> {
        self.request_panicking(Send(qos, port, data)).await
    }

    /// Send data on a specific port with a given quality of service. If the LoRa module receives
    /// any data as part of the confirmation, write it into the provided buffer and return the size of the data read.
    pub async fn send_recv<'a>(
        &self,
        qos: QoS,
        port: Port,
        data: &'a [u8],
        rx: &'a mut [u8],
    ) -> Result<usize, LoraError> {
        self.request_panicking(SendRecv(qos, port, data, rx)).await
    }
}

#[derive(Debug)]
pub enum LoraError {
    SendError,
    RecvError,
    RecvTimeout,
    RecvBufferTooSmall,
    NotInitialized,
    NotImplemented,
    OtherError,
}

/// Trait for a LoRa driver.
#[allow(clippy::needless_lifetimes)]
pub trait LoraDriver: Actor {
    /// Configure the LoRa module.
    fn configure<'a>(self, message: Configure<'a>) -> Response<Self, Result<(), LoraError>>;

    /// Perform a reset of the LoRa module, retaining configuration previously applied.
    fn reset(self, message: Reset) -> Response<Self, Result<(), LoraError>>;

    /// Join a LoRaWAN network using the specified connect mode.
    fn join(self, message: Join) -> Response<Self, Result<(), LoraError>>;

    /// Send data on a specific port with a given quality of service.
    fn send<'a>(self, message: Send<'a>) -> Response<Self, Result<(), LoraError>>;

    /// Send data on a specific port with a given quality of service. If the LoRa module receives
    /// any data as part of the confirmation, the command provides a buffer that the implementation
    /// may write the data into. The number of bytes read should be returned.
    fn send_recv<'a>(self, message: SendRecv<'a>) -> Response<Self, Result<usize, LoraError>>;
}

/// Message types and handlers for the LoraDriver trait.

#[derive(Debug)]
pub struct Configure<'a>(pub &'a LoraConfig);
#[derive(Debug)]
pub struct Join(pub ConnectMode);
#[derive(Debug)]
pub struct Reset(pub ResetMode);
#[derive(Debug)]
pub struct Send<'a>(pub QoS, pub Port, pub &'a [u8]);
#[derive(Debug)]
pub struct SendRecv<'a>(pub QoS, pub Port, pub &'a [u8], pub &'a mut [u8]);

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

impl<'a, A> RequestHandler<SendRecv<'a>> for A
where
    A: LoraDriver,
{
    type Response = Result<usize, LoraError>;
    fn on_request(self, message: SendRecv<'a>) -> Response<Self, Self::Response> {
        self.send_recv(message)
    }
}
