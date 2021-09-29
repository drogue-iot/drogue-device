///A network driver for a RAK811 attached via a UART.
///
///Currently requires the RAK811 to be flashed with a 2.x version of the AT firmware.
///
mod buffer;
mod parser;
mod protocol;
use crate::{
    kernel::actor::{Actor, Address, Inbox},
    traits::lora::*,
};

pub use buffer::*;
use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicBool, Ordering},
};
use embassy::{
    blocking_mutex::kind::ThreadMode,
    channel::{
        mpsc::{self, Channel, Receiver, Sender},
        signal::Signal,
    },
    io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt},
};
use embedded_hal::digital::v2::OutputPin;
use futures::future::{select, Either};
use futures::pin_mut;
pub use protocol::*;

const RECV_BUFFER_LEN: usize = 256;
type DriverMutex = ThreadMode;

pub struct Initialized {
    signal: Signal<Result<LoraRegion, LoraError>>,
    initialized: AtomicBool,
}

impl Initialized {
    pub fn new() -> Self {
        Self {
            signal: Signal::new(),
            initialized: AtomicBool::new(false),
        }
    }

    async fn wait(&self) -> Result<Option<LoraRegion>, LoraError> {
        if self.initialized.swap(true, Ordering::SeqCst) == false {
            let region = self.signal.wait().await?;
            return Ok(Some(region));
        }
        Ok(None)
    }

    pub fn signal(&self, result: Result<LoraRegion, LoraError>) {
        self.signal.signal(result);
    }
}

pub struct Rak811Driver {
    initialized: Initialized,
    command_channel: Channel<DriverMutex, CommandBuffer, 2>,
    response_channel: Channel<DriverMutex, Response, 2>,
}

pub struct Rak811Controller<'a> {
    config: LoraConfig,
    initialized: &'a Initialized,
    command_producer: Sender<'a, DriverMutex, CommandBuffer, 2>,
    response_consumer: Receiver<'a, DriverMutex, Response, 2>,
}

pub struct Rak811Modem<'a, UART, RESET>
where
    UART: AsyncBufRead + AsyncBufReadExt + AsyncWrite + AsyncWriteExt + 'static,
    RESET: OutputPin,
{
    initialized: &'a Initialized,
    uart: UART,
    reset: RESET,
    parse_buffer: Buffer,
    command_consumer: Receiver<'a, DriverMutex, CommandBuffer, 2>,
    response_producer: Sender<'a, DriverMutex, Response, 2>,
}

impl Rak811Driver {
    pub fn new() -> Self {
        Self {
            initialized: Initialized::new(),
            command_channel: Channel::new(),
            response_channel: Channel::new(),
        }
    }

    pub fn initialize<'a, UART, RESET>(
        &'a mut self,
        uart: UART,
        reset: RESET,
    ) -> (Rak811Controller<'a>, Rak811Modem<'a, UART, RESET>)
    where
        UART: AsyncBufRead + AsyncBufReadExt + AsyncWrite + AsyncWriteExt + 'static,
        RESET: OutputPin + 'static,
    {
        let (cp, cc) = mpsc::split(&mut self.command_channel);
        let (rp, rc) = mpsc::split(&mut self.response_channel);

        let modem = Rak811Modem::new(&self.initialized, uart, reset, cc, rp);
        let controller = Rak811Controller::new(&self.initialized, cp, rc);

        (controller, modem)
    }
}

impl<'a, UART, RESET> Rak811Modem<'a, UART, RESET>
where
    UART: AsyncBufRead + AsyncBufReadExt + AsyncWrite + AsyncWriteExt + 'static,
    RESET: OutputPin + 'static,
{
    pub fn new(
        initialized: &'a Initialized,
        uart: UART,
        reset: RESET,
        command_consumer: Receiver<'a, DriverMutex, CommandBuffer, 2>,
        response_producer: Sender<'a, DriverMutex, Response, 2>,
    ) -> Self {
        Self {
            initialized,
            uart,
            reset,
            parse_buffer: Buffer::new(),
            command_consumer,
            response_producer,
        }
    }

    async fn initialize(&mut self) -> Result<LoraRegion, LoraError> {
        self.reset.set_high().ok();
        self.reset.set_low().ok();
        loop {
            // Run processing to increase likelyhood we have something to parse.
            self.process().await?;
            if let Some(response) = self.parse() {
                match response {
                    Response::Initialized(region) => {
                        info!("Got initialize response with region {:?}", region);
                        return Ok(region);
                    }
                    e => {
                        error!("Got unexpected repsonse: {:?}", e);
                        return Err(LoraError::NotInitialized);
                    }
                }
            }
        }
    }

    async fn process(&mut self) -> Result<(), LoraError> {
        let mut buf = [0; 1];
        let mut uart = unsafe { Pin::new_unchecked(&mut self.uart) };
        let len = uart
            .read(&mut buf[..])
            .await
            .map_err(|_| LoraError::RecvError)?;
        if len > 0 {
            self.parse_buffer
                .write(buf[0])
                .map_err(|_| LoraError::RecvError)?;
        }
        Ok(())
    }

    fn parse(&mut self) -> Option<Response> {
        let result = self.parse_buffer.parse();
        if let Ok(response) = result {
            if !matches!(response, Response::None) {
                debug!("Got response: {:?}", response);
                return Some(response);
            }
        }
        None
    }

    async fn digest(&mut self) {
        if let Some(response) = self.parse() {
            let _ = self.response_producer.send(response).await;
        }
    }

    pub async fn run(&mut self) -> ! {
        let result = self.initialize().await;
        self.initialized.signal(result);
        loop {
            let mut buf = [0; 1];
            let (cmd, input) = {
                let command_fut = self.command_consumer.recv();
                let mut uart = unsafe { Pin::new_unchecked(&mut self.uart) };
                let uart_fut = uart.read(&mut buf[..]);
                pin_mut!(uart_fut);

                match select(command_fut, uart_fut).await {
                    Either::Left((s, _)) => (Some(s), None),
                    Either::Right((r, _)) => (None, Some(r)),
                }
            };
            // We got command to write, write it
            if let Some(Some(s)) = cmd {
                let mut uart = unsafe { Pin::new_unchecked(&mut self.uart) };
                if let Err(e) = uart.write_all(s.as_bytes()).await {
                    error!("Error writing command to uart: {:?}", e);
                }
            }

            // We got input, digest it
            if let Some(input) = input {
                match input {
                    Ok(len) => {
                        for b in &buf[..len] {
                            self.parse_buffer.write(*b).unwrap();
                        }
                        self.digest().await;
                    }
                    Err(e) => {
                        error!("Error reading from uart: {:?}", e);
                    }
                }
            }
        }
    }
}

/*
    /// Send reset command to lora module. Depending on the mode, this will restart
    /// the module or reload its configuration from EEPROM.
    pub fn reset(&mut self, mode: ResetMode) -> Result<(), DriverError> {
        let response = self.send_command(Command::Reset(mode))?;
        match response {
            Response::Ok => {
                let response = self.recv_response()?;
                match response {
                    Response::Initialized(band) => {
                        self.lora_band = band;
                        Ok(())
                    }
                    _ => Err(DriverError::NotInitialized),
                }
            }
            r => log_unexpected(r),
        }
    }
*/

fn log_unexpected(r: Response) -> Result<(), LoraError> {
    error!("Unexpected response: {:?}", r);
    Err(LoraError::OtherError)
}

impl<'a> LoraDriver for Rak811Controller<'a> {
    #[rustfmt::skip]
    type JoinFuture<'m> where 'a: 'm = impl Future<Output = Result<(), LoraError>> + 'm;
    fn join<'m>(&'m mut self, mode: JoinMode) -> Self::JoinFuture<'m> {
        async move {
            let mode = match mode {
                JoinMode::OTAA {
                    dev_eui,
                    app_eui,
                    app_key,
                } => {
                    self.send_command_ok(Command::SetConfig(ConfigOption::DevEui(&dev_eui)))
                        .await?;
                    self.send_command_ok(Command::SetConfig(ConfigOption::AppEui(&app_eui)))
                        .await?;
                    self.send_command_ok(Command::SetConfig(ConfigOption::AppKey(&app_key)))
                        .await?;
                    ConnectMode::OTAA
                }
                JoinMode::ABP {
                    news_key,
                    apps_key,
                    dev_addr,
                } => {
                    self.send_command_ok(Command::SetConfig(ConfigOption::DevAddr(&dev_addr)))
                        .await?;
                    self.send_command_ok(Command::SetConfig(ConfigOption::AppsKey(&apps_key)))
                        .await?;
                    self.send_command_ok(Command::SetConfig(ConfigOption::NwksKey(&news_key)))
                        .await?;
                    ConnectMode::ABP
                }
            };
            let response = self.send_command(Command::Join(mode)).await?;
            match response {
                Response::Ok => {
                    let response = self.response_consumer.recv().await.unwrap();
                    match response {
                        Response::Recv(EventCode::JoinedSuccess, _, _, _) => Ok(()),
                        r => log_unexpected(r),
                    }
                }
                r => log_unexpected(r),
            }
        }
    }

    #[rustfmt::skip]
    type SendFuture<'m> where 'a: 'm = impl Future<Output = Result<(), LoraError>> + 'm;
    fn send<'m>(&'m mut self, qos: QoS, port: Port, data: &'m [u8]) -> Self::SendFuture<'m> {
        async move {
            let response = self.send_command(Command::Send(qos, port, data)).await?;
            match response {
                Response::Ok => {
                    let response = self.response_consumer.recv().await.unwrap();
                    let expected_code = match qos {
                        QoS::Unconfirmed => EventCode::TxUnconfirmed,
                        QoS::Confirmed => EventCode::TxConfirmed,
                    };
                    match response {
                        Response::Recv(c, 0, _, _) if expected_code == c => Ok(()),
                        r => log_unexpected(r),
                    }
                }
                r => log_unexpected(r),
            }
        }
    }

    #[rustfmt::skip]
    type SendRecvFuture<'m> where 'a: 'm = impl Future<Output = Result<usize, LoraError>> + 'm;
    fn send_recv<'m>(
        &'m mut self,
        qos: QoS,
        port: Port,
        data: &'m [u8],
        rx: &'m mut [u8],
    ) -> Self::SendRecvFuture<'m> {
        async move { Ok(0) }
    }
}

impl<'a> Rak811Controller<'a> {
    pub fn new(
        initialized: &'a Initialized,
        command_producer: Sender<'a, DriverMutex, CommandBuffer, 2>,
        response_consumer: Receiver<'a, DriverMutex, Response, 2>,
    ) -> Self {
        Self {
            config: LoraConfig::new(),
            initialized,
            command_producer,
            response_consumer,
        }
    }

    async fn send_command<'m>(&mut self, command: Command<'m>) -> Result<Response, LoraError> {
        if let Some(region) = self.initialized.wait().await? {
            self.config.region.replace(region);
        }
        let mut s = Command::buffer();
        command.encode(&mut s);
        debug!("Sending command {}", s.as_str());
        s.push_str("\r\n").unwrap();
        let _ = self.command_producer.send(s).await;

        let response = self.response_consumer.recv().await;
        Ok(response.unwrap())
    }

    async fn send_command_ok<'m>(&mut self, command: Command<'m>) -> Result<(), LoraError> {
        match self.send_command(command).await? {
            Response::Ok => Ok(()),
            r => Err(LoraError::OtherError),
        }
    }

    pub async fn configure(&mut self, config: &LoraConfig) -> Result<(), LoraError> {
        if let Some(region) = self.initialized.wait().await? {
            self.config.region.replace(region);
        }
        info!("Applying config: {:?}", config);
        if let Some(region) = config.region {
            if self.config.region != config.region {
                self.send_command_ok(Command::SetBand(region)).await?;
                self.config.region.replace(region);
            }
        }
        if let Some(lora_mode) = config.lora_mode {
            if self.config.lora_mode != config.lora_mode {
                self.send_command_ok(Command::SetMode(lora_mode)).await?;
                self.config.lora_mode.replace(lora_mode);
            }
        }
        debug!("Config applied");
        Ok(())
    }
}

/// Convenience actor implementation of modem
pub struct Rak811ModemActor<'a, UART, RESET>
where
    UART: AsyncBufRead + AsyncBufReadExt + AsyncWrite + AsyncWriteExt + 'static,
    RESET: OutputPin + 'static,
{
    modem: Option<Rak811Modem<'a, UART, RESET>>,
}

impl<'a, UART, RESET> Rak811ModemActor<'a, UART, RESET>
where
    UART: AsyncBufRead + AsyncBufReadExt + AsyncWrite + AsyncWriteExt + 'static,
    RESET: OutputPin + 'static,
{
    pub fn new() -> Self {
        Self { modem: None }
    }
}

impl<'a, UART, RESET> Unpin for Rak811ModemActor<'a, UART, RESET>
where
    UART: AsyncBufRead + AsyncBufReadExt + AsyncWrite + AsyncWriteExt + 'static,
    RESET: OutputPin + 'static,
{
}

impl<'a, UART, RESET> Actor for Rak811ModemActor<'a, UART, RESET>
where
    UART: AsyncBufRead + AsyncBufReadExt + AsyncWrite + AsyncWriteExt + 'static,
    RESET: OutputPin + 'static,
{
    type Configuration = Rak811Modem<'a, UART, RESET>;
    #[rustfmt::skip]
    type Message<'m> where 'a: 'm = ();

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where 'a: 'm, M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        _: Address<'static, Self>,
        _: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        self.modem.replace(config);
        async move {
            loop {
                self.modem.as_mut().unwrap().run().await;
            }
        }
    }
}
