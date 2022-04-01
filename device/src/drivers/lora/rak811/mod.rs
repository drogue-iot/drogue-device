///A network driver for a RAK811 attached via a UART.
///
///Currently requires the RAK811 to be flashed with a 2.x version of the AT firmware.
///
mod buffer;
mod parser;
mod protocol;
use crate::traits::lora::*;
use drogue_actor::{Actor, Address, Inbox};

pub use buffer::*;
use core::{
    future::Future,
    sync::atomic::{AtomicBool, Ordering},
};
use embassy::{
    blocking_mutex::raw::NoopRawMutex,
    channel::{
        mpsc::{self, Channel, Receiver, Sender},
        signal::Signal,
    },
};
use embedded_hal::digital::v2::OutputPin;
use embedded_hal_async::serial::{Read, Write};
pub use protocol::*;

const RECV_BUFFER_LEN: usize = 256;
type DriverMutex = NoopRawMutex;

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
    response_channel: Channel<DriverMutex, Response, 2>,
}

pub struct Rak811Controller<'a, TX>
where
    TX: Write + 'static,
{
    config: LoraConfig,
    initialized: &'a Initialized,
    response_consumer: Receiver<'a, DriverMutex, Response, 2>,
    tx: TX,
}

pub struct Rak811Modem<'a, RX, RESET>
where
    RX: Read + 'static,
    RESET: OutputPin,
{
    initialized: &'a Initialized,
    rx: RX,
    reset: RESET,
    parse_buffer: Buffer,
    response_producer: Sender<'a, DriverMutex, Response, 2>,
}

impl Rak811Driver {
    pub fn new() -> Self {
        Self {
            initialized: Initialized::new(),
            response_channel: Channel::new(),
        }
    }

    pub fn initialize<'a, TX, RX, RESET>(
        &'a mut self,
        tx: TX,
        rx: RX,
        reset: RESET,
    ) -> (Rak811Controller<'a, TX>, Rak811Modem<'a, RX, RESET>)
    where
        TX: Write + 'static,
        RX: Read + 'static,
        RESET: OutputPin + 'static,
    {
        let (rp, rc) = mpsc::split(&mut self.response_channel);

        let modem = Rak811Modem::new(&self.initialized, rx, reset, rp);
        let controller = Rak811Controller::new(&self.initialized, tx, rc);

        (controller, modem)
    }
}

impl<'a, RX, RESET> Rak811Modem<'a, RX, RESET>
where
    RX: Read + 'static,
    RESET: OutputPin + 'static,
{
    pub fn new(
        initialized: &'a Initialized,
        rx: RX,
        reset: RESET,
        response_producer: Sender<'a, DriverMutex, Response, 2>,
    ) -> Self {
        Self {
            initialized,
            rx,
            reset,
            parse_buffer: Buffer::new(),
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
        self.rx
            .read(&mut buf[..])
            .await
            .map_err(|_| LoraError::RecvError)?;
        self.parse_buffer
            .write(buf[0])
            .map_err(|_| LoraError::RecvError)?;
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
            match self.rx.read(&mut buf[..]).await {
                Ok(()) => {
                    for b in &buf[..] {
                        self.parse_buffer.write(*b).unwrap();
                    }
                    self.digest().await;
                }
                Err(_) => {
                    error!("Error reading from uart");
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

impl<'a, TX> LoraDriver for Rak811Controller<'a, TX>
where
    TX: Write,
{
    type JoinFuture<'m> = impl Future<Output = Result<(), LoraError>> + 'm
    where
        Self: 'm,
        'a: 'm;
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

    type SendFuture<'m> = impl Future<Output = Result<(), LoraError>> + 'm
    where
        Self: 'm,
        'a: 'm;
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

    type SendRecvFuture<'m> = impl Future<Output = Result<usize, LoraError>> + 'm
    where
        Self: 'm,
        'a: 'm;
    fn send_recv<'m>(
        &'m mut self,
        _qos: QoS,
        _port: Port,
        _data: &'m [u8],
        _rx: &'m mut [u8],
    ) -> Self::SendRecvFuture<'m> {
        async move { todo!() }
    }
}

impl<'a, TX> Rak811Controller<'a, TX>
where
    TX: Write,
{
    pub fn new(
        initialized: &'a Initialized,
        tx: TX,
        response_consumer: Receiver<'a, DriverMutex, Response, 2>,
    ) -> Self {
        Self {
            config: LoraConfig::new(),
            initialized,
            tx,
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
        self.tx
            .write(s.as_bytes())
            .await
            .map_err(|_| LoraError::SendError)?;

        let response = self.response_consumer.recv().await;
        Ok(response.unwrap())
    }

    async fn send_command_ok<'m>(&mut self, command: Command<'m>) -> Result<(), LoraError> {
        match self.send_command(command).await? {
            Response::Ok => Ok(()),
            _ => Err(LoraError::OtherError),
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
pub struct Rak811ModemActor<'a, RX, RESET>
where
    RX: Read + 'static,
    RESET: OutputPin + 'static,
{
    modem: Rak811Modem<'a, RX, RESET>,
}

impl<'a, RX, RESET> Rak811ModemActor<'a, RX, RESET>
where
    RX: Read + 'static,
    RESET: OutputPin + 'static,
{
    pub fn new(modem: Rak811Modem<'a, RX, RESET>) -> Self {
        Self { modem }
    }
}

impl<'a, RX, RESET> Unpin for Rak811ModemActor<'a, RX, RESET>
where
    RX: Read + 'static,
    RESET: OutputPin + 'static,
{
}

impl<'a, RX, RESET> Actor for Rak811ModemActor<'a, RX, RESET>
where
    RX: Read + 'static,
    RESET: OutputPin + 'static,
{
    type Message<'m> = ()
    where
        Self: 'm,
        'a: 'm;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        'a: 'm,
        Self: 'm,
        M: 'm + Inbox<Self>;
    fn on_mount<'m, M>(&'m mut self, _: Address<Self>, _: &'m mut M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            loop {
                self.modem.run().await;
            }
        }
    }
}
