use crate::domain::time::duration::Milliseconds;
use crate::driver::lora::*;
use crate::driver::uart::dma;
use crate::hal::timer::Timer as HalTimer;
use crate::hal::uart::{DmaUart, Error as UartError};
use crate::handler::{RequestHandler, Response};
use crate::prelude::*;

use drogue_rak811::{Buffer, Command, ConfigOption, DriverError, Response as RakResponse};
use embedded_hal::digital::v2::OutputPin;
//use heapless::{consts, spsc::Queue};

type Uart<U, T> = <dma::Uart<U, T> as Package>::Primary;
pub struct Rak811<U, T, RST>
where
    U: DmaUart + 'static,
    T: HalTimer + 'static,
    RST: OutputPin,
{
    uart: Option<Address<Uart<U, T>>>,
    parse_buffer: Buffer,
    config: LoraConfig,
    rst: RST,
}

impl<U, T, RST> Rak811<U, T, RST>
where
    U: DmaUart,
    T: HalTimer,
    RST: OutputPin,
{
    pub fn new(rst: RST) -> Self {
        Self {
            uart: None,
            //            rxq: Queue::new(),
            parse_buffer: Buffer::new(),
            config: LoraConfig::new(),
            rst,
        }
    }

    async fn send_command<'a, 'b>(
        &mut self,
        command: Command<'a>,
    ) -> Result<RakResponse, DriverError>
    where
        U: DmaUart,
    {
        let mut s = Command::buffer();
        command.encode(&mut s);
        log::debug!("Sending command {}", s.as_str());

        {
            let uart = self.uart.as_ref().unwrap();
            uart.write(s.as_bytes()).await?;
            uart.write(b"\r\n").await?;
        }

        log::debug!("Awaiting response");
        let response = self.recv_response().await;

        log::debug!("Got response: {:?}", response);
        response
    }

    async fn recv_response<'b>(&mut self) -> Result<RakResponse, DriverError>
    where
        U: DmaUart,
    {
        loop {
            // Run processing to increase likelyhood we have something to parse.
            self.process().await?;
            if let Some(response) = self.digest() {
                return Ok(response);
            }
        }
    }

    fn digest(&mut self) -> Option<RakResponse> {
        let result = self.parse_buffer.parse();
        if let Ok(response) = result {
            if !matches!(response, RakResponse::None) {
                return Some(response);
            }
        }
        None
    }

    async fn process<'b>(&mut self) -> Result<(), DriverError>
    where
        U: DmaUart,
    {
        let uart = self.uart.as_ref().unwrap();
        let mut rx_buf: [u8; 8] = [0; 8];

        let len = uart
            .read_with_timeout(&mut rx_buf[..], Milliseconds(1000))
            .await?;

        /*
        let timer = self.timer.as_ref().unwrap();
            pin_mut!(read);
            pin_mut!(timeout);
            match select(read, timeout).await {
                Either::Left((result, _)) => {
                    log::info!("Read result {:?}", result);
                    if let Ok(len) = result {
                        len
                    } else {
                        0
                    }
                }
                Either::Right((_, read)) => {
                    log::info!("TIMEOUT");
                    // uart.cancel();
                    let result = read.await;
                    match result {
                        Ok(len) => len,
                        _ => 0,
                    }
                }
            }
        };*/

        for b in &mut rx_buf[..len] {
            self.parse_buffer.write(*b).unwrap();
        }

        Ok(())
    }

    async fn send_command_ok<'a, 'b>(&mut self, command: Command<'a>) -> Result<(), LoraError>
    where
        U: DmaUart,
    {
        let response = self.send_command(command).await;
        match response {
            Ok(RakResponse::Ok) => Ok(()),
            Ok(r) => Err(DriverError::UnexpectedResponse.into()),
            Err(e) => Err(e.into()),
        }
    }

    async fn apply_config<'b>(&mut self) -> Result<(), LoraError>
    where
        U: DmaUart,
    {
        let config = self.config;
        log::debug!("Applying config: {:?}", config);
        if let Some(band) = config.band {
            self.send_command_ok(Command::SetBand(band)).await?;
        }

        if let Some(lora_mode) = config.lora_mode {
            self.send_command_ok(Command::SetMode(lora_mode)).await?;
        }

        if let Some(ref device_address) = config.device_address {
            self.send_command_ok(Command::SetConfig(ConfigOption::DevAddr(device_address)))
                .await?;
        }

        if let Some(ref device_eui) = config.device_eui {
            self.send_command_ok(Command::SetConfig(ConfigOption::DevEui(device_eui)))
                .await?;
        }

        if let Some(ref app_eui) = config.app_eui {
            self.send_command_ok(Command::SetConfig(ConfigOption::AppEui(app_eui)))
                .await?;
        }

        if let Some(ref app_key) = config.app_key {
            self.send_command_ok(Command::SetConfig(ConfigOption::AppKey(app_key)))
                .await?;
        }
        Ok(())
    }
}

impl<U, T, RST> Actor for Rak811<U, T, RST>
where
    U: DmaUart,
    T: HalTimer,
    RST: OutputPin,
{
    type Configuration = Address<Uart<U, T>>;
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        self.uart.replace(config);
    }
}

impl<'a, U, T, RST> RequestHandler<Initialize> for Rak811<U, T, RST>
where
    U: DmaUart,
    T: HalTimer,
    RST: OutputPin,
{
    type Response = Result<(), LoraError>;
    fn on_request(mut self, message: Initialize) -> Response<Self, Self::Response> {
        log::info!("Initialize!");
        Response::defer(async move {
            self.rst.set_high().ok();
            self.rst.set_low().ok();
            let response = self.recv_response().await;
            log::info!("INitilize response: {:?}", response);
            let result = match response {
                Ok(RakResponse::Initialized) => Ok(()),
                _ => Err(LoraError),
            };
            (self, result)
        })
    }
}

impl<'a, U, T, RST> RequestHandler<Reset> for Rak811<U, T, RST>
where
    U: DmaUart,
    T: HalTimer,
    RST: OutputPin,
{
    type Response = Result<(), LoraError>;
    fn on_request(mut self, message: Reset) -> Response<Self, Self::Response> {
        Response::defer(async move {
            let response = self.send_command(Command::Reset(message.0)).await;
            let result = match response {
                Ok(RakResponse::Ok) => {
                    let response = self.recv_response().await;
                    match response {
                        Ok(RakResponse::Initialized) => Ok(()),
                        _ => Err(DriverError::NotInitialized.into()),
                    }
                }
                Ok(r) => Err(DriverError::UnexpectedResponse.into()),
                Err(e) => Err(e.into()),
            };
            (self, result)
        })
    }
}

impl<'a, U, T, RST> RequestHandler<Configure<'a>> for Rak811<U, T, RST>
where
    U: DmaUart,
    T: HalTimer,
    RST: OutputPin,
{
    type Response = Result<(), LoraError>;
    fn on_request(mut self, message: Configure<'a>) -> Response<Self, Self::Response> {
        self.config = message.0.clone();
        Response::defer(async move {
            let result = self.apply_config().await;
            (self, result)
        })
    }
}

impl<'a, U, T, RST> RequestHandler<Join> for Rak811<U, T, RST>
where
    U: DmaUart,
    T: HalTimer,
    RST: OutputPin,
{
    type Response = Result<(), LoraError>;
    fn on_request(self, message: Join) -> Response<Self, Self::Response> {
        Response::immediate(self, Ok(()))
    }
}

impl<'a, U, T, RST> RequestHandler<Send<'a>> for Rak811<U, T, RST>
where
    U: DmaUart,
    T: HalTimer,
    RST: OutputPin,
{
    type Response = Result<(), LoraError>;
    fn on_request(self, message: Send<'a>) -> Response<Self, Self::Response> {
        Response::immediate(self, Ok(()))
    }
}

impl core::convert::From<UartError> for DriverError {
    fn from(error: UartError) -> Self {
        match error {
            UartError::TxInProgress
            | UartError::TxBufferTooSmall
            | UartError::TxBufferTooLong
            | UartError::Transmit => DriverError::WriteError,
            UartError::RxInProgress
            | UartError::RxBufferTooSmall
            | UartError::RxBufferTooLong
            | UartError::Receive => DriverError::ReadError,
            _ => DriverError::OtherError,
        }
    }
}

impl core::convert::From<DriverError> for LoraError {
    fn from(error: DriverError) -> Self {
        LoraError
    }
}
