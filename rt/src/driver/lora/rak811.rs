use crate::api::{
    lora::*,
    queue::*,
    uart::{Error as UartError, UartReader, UartWriter},
};
use crate::driver::queue::spsc_queue::*;
use crate::prelude::*;

use drogue_rak811::{
    Buffer, Command, ConfigOption, DriverError, EventCode, Response as RakResponse,
};
use embedded_hal::digital::v2::OutputPin;
use heapless::{consts, String};

type QueueActor = <SpscQueue<RakResponse, consts::U2> as Package>::Primary;

pub struct Rak811Actor<U, Q, RST>
where
    U: UartWriter + 'static,
    Q: Queue<T = RakResponse> + 'static,
    RST: OutputPin + 'static,
{
    uart: Option<Address<U>>,
    command_buffer: String<consts::U128>,
    config: LoraConfig,
    rst: RST,
    response_queue: Option<Address<Q>>,
}
pub struct Rak811Ingress<U, Q>
where
    U: UartReader + 'static,
    Q: Queue<T = RakResponse> + 'static,
{
    uart: Option<Address<U>>,
    parse_buffer: Buffer,
    response_queue: Option<Address<Q>>,
}

pub struct Rak811<U, RST>
where
    U: UartReader + UartWriter + 'static,
    RST: OutputPin + 'static,
{
    actor: ActorContext<Rak811Actor<U, QueueActor, RST>>,
    ingress: ActorContext<Rak811Ingress<U, QueueActor>>,
    response_queue: SpscQueue<RakResponse, consts::U2>,
}

impl<U, RST> Package for Rak811<U, RST>
where
    U: UartReader + UartWriter + 'static,
    RST: OutputPin + 'static,
{
    type Primary = Rak811Actor<U, QueueActor, RST>;
    type Configuration = Address<U>;
    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary>
    where
        Self: 'static,
    {
        let queue = self.response_queue.mount((), supervisor);
        self.ingress.mount((queue, config), supervisor);
        let addr = self.actor.mount((queue, config), supervisor);

        addr
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.actor.address()
    }
}

impl<U, RST> Rak811<U, RST>
where
    U: UartReader + UartWriter + 'static,
    RST: OutputPin,
{
    pub fn new(rst: RST) -> Self {
        Self {
            actor: ActorContext::new(Rak811Actor::new(rst)).with_name("rak811_actor"),
            ingress: ActorContext::new(Rak811Ingress::new()).with_name("rak811_ingress"),
            response_queue: SpscQueue::new(),
        }
    }
}

impl<U, Q, RST> Rak811Actor<U, Q, RST>
where
    U: UartWriter,
    Q: Queue<T = RakResponse> + 'static,
    RST: OutputPin,
{
    pub fn new(rst: RST) -> Self {
        Self {
            uart: None,
            command_buffer: String::new(),
            config: LoraConfig::new(),
            rst,
            response_queue: None,
        }
    }

    fn encode_command<'b>(&mut self, command: Command<'b>) {
        let s = &mut self.command_buffer;
        s.clear();
        command.encode(s);
        s.push_str("\r\n").unwrap();
    }

    async fn send_command(&mut self) -> Result<RakResponse, LoraError> {
        let s = &mut self.command_buffer;

        log::debug!("Sending command {}", s.as_str());
        let uart = self.uart.as_ref().unwrap();

        uart.write(s.as_bytes()).await?;

        self.recv_response().await
    }

    async fn recv_response(&mut self) -> Result<RakResponse, LoraError>
where {
        let r = self
            .response_queue
            .as_ref()
            .unwrap()
            .dequeue()
            .await
            .map_err(|_| LoraError::RecvError);
        r
    }

    async fn encode_send_command<'b>(
        &mut self,
        command: Command<'b>,
    ) -> Result<RakResponse, LoraError> {
        self.encode_command(command);
        self.send_command().await
    }

    async fn encode_send_command_ok<'b>(&mut self, command: Command<'b>) -> Result<(), LoraError> {
        self.encode_command(command);
        match self.send_command().await? {
            RakResponse::Ok => Ok(()),
            r => Err(LoraError::OtherError),
        }
    }

    async fn apply_config(&mut self, config: LoraConfig) -> Result<(), LoraError> {
        log::debug!("Applying config: {:?}", config);
        if let Some(band) = config.band {
            if self.config.band != config.band {
                self.encode_send_command_ok(Command::SetBand(band)).await?;
                self.config.band.replace(band);
            }
        }
        if let Some(lora_mode) = config.lora_mode {
            if self.config.lora_mode != config.lora_mode {
                self.encode_send_command_ok(Command::SetMode(lora_mode))
                    .await?;
                self.config.lora_mode.replace(lora_mode);
            }
        }

        if let Some(ref device_address) = config.device_address {
            self.encode_send_command_ok(Command::SetConfig(ConfigOption::DevAddr(device_address)))
                .await?;
            self.config.device_address.replace(*device_address);
        }

        if let Some(ref device_eui) = config.device_eui {
            self.encode_send_command_ok(Command::SetConfig(ConfigOption::DevEui(device_eui)))
                .await?;
            self.config.device_eui.replace(*device_eui);
        }

        if let Some(ref app_eui) = config.app_eui {
            self.encode_send_command_ok(Command::SetConfig(ConfigOption::AppEui(app_eui)))
                .await?;
            self.config.app_eui.replace(*app_eui);
        }

        if let Some(ref app_key) = config.app_key {
            self.encode_send_command_ok(Command::SetConfig(ConfigOption::AppKey(app_key)))
                .await?;
            self.config.app_key.replace(*app_key);
        }

        log::debug!("Config applied");
        Ok(())
    }
}

impl<U, Q, RST> Actor for Rak811Actor<U, Q, RST>
where
    U: UartWriter,
    Q: Queue<T = RakResponse> + 'static,
    RST: OutputPin,
{
    type Configuration = (Address<Q>, Address<U>);
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        self.response_queue.replace(config.0);
        self.uart.replace(config.1);
    }

    fn on_initialize(mut self) -> Completion<Self> {
        Completion::defer(async move {
            log::debug!("RAK811 LoRa module initializing");
            self.rst.set_low().ok();
            let response = self.recv_response().await;
            match response {
                Ok(RakResponse::Initialized(band)) => {
                    self.config.band.replace(band);
                    log::info!(
                        "[{}] RAK811 driver initialized with band {:?}",
                        ActorInfo::name(),
                        band
                    );
                }
                Ok(r) => {
                    log::error!(
                        "Unexpected response when initializing RAK811 driver: {:?}",
                        r
                    );
                }
                Err(e) => {
                    log::error!("Error initializing RAK811 driver: {:?}", e);
                }
            }
            self
        })
    }
}

impl<U, Q, RST> LoraDriver for Rak811Actor<U, Q, RST>
where
    U: UartWriter,
    Q: Queue<T = RakResponse> + 'static,
    RST: OutputPin,
{
    fn reset(mut self, message: Reset) -> Response<Self, Result<(), LoraError>> {
        Response::defer(async move {
            let response = self.encode_send_command(Command::Reset(message.0)).await;
            let result = match response {
                Ok(RakResponse::Ok) => {
                    let response = self.recv_response().await;
                    match response {
                        Ok(RakResponse::Initialized(band)) => {
                            self.config.band.replace(band);
                            Ok(())
                        }
                        _ => Err(LoraError::NotInitialized),
                    }
                }
                r => Err(LoraError::OtherError),
            };
            (self, result)
        })
    }

    fn configure<'a>(mut self, message: Configure<'a>) -> Response<Self, Result<(), LoraError>> {
        let config = message.0.clone();
        Response::defer(async move {
            let result = self.apply_config(config).await;
            (self, result)
        })
    }

    fn join(mut self, message: Join) -> Response<Self, Result<(), LoraError>> {
        Response::defer(async move {
            let result = self.encode_send_command_ok(Command::Join(message.0)).await;
            let response = match result {
                Ok(_) => {
                    let response = self.recv_response().await;
                    match response {
                        Ok(RakResponse::Recv(EventCode::JoinedSuccess, _, _, _)) => Ok(()),
                        r => {
                            log::debug!("Received response: {:?}", r);
                            Err(LoraError::OtherError)
                        }
                    }
                }
                r => {
                    log::debug!("Received response: {:?}", r);
                    Err(LoraError::OtherError)
                }
            };
            (self, response)
        })
    }

    fn send<'a>(mut self, message: Send<'a>) -> Response<Self, Result<(), LoraError>> {
        let result = self.encode_command(Command::Send(message.0, message.1, message.2));
        let expected_code = match message.0 {
            QoS::Unconfirmed => EventCode::TxUnconfirmed,
            QoS::Confirmed => EventCode::TxConfirmed,
        };
        Response::defer(async move {
            let result = match self.send_command().await {
                Ok(RakResponse::Ok) => match self.recv_response().await {
                    Ok(RakResponse::Recv(c, 0, _, _)) if expected_code == c => Ok(()),
                    r => {
                        log::error!("Unexpected response: {:?}", r);
                        Err(LoraError::OtherError)
                    }
                },
                r => {
                    log::error!("Unexpected response: {:?}", r);
                    Err(LoraError::OtherError)
                }
            };
            (self, result)
        })
    }

    fn send_recv<'a>(mut self, message: SendRecv<'a>) -> Response<Self, Result<usize, LoraError>> {
        Response::immediate(self, Err(LoraError::NotImplemented))
    }
}

impl<U, Q> Rak811Ingress<U, Q>
where
    U: UartReader,
    Q: Queue<T = RakResponse> + 'static,
{
    pub fn new() -> Self {
        Self {
            uart: None,
            response_queue: None,
            parse_buffer: Buffer::new(),
        }
    }

    async fn digest(&mut self) -> Result<(), LoraError> {
        let result = self.parse_buffer.parse();
        if let Ok(response) = result {
            if !matches!(response, RakResponse::None) {
                self.response_queue
                    .as_ref()
                    .unwrap()
                    .enqueue(response)
                    .await
                    .map_err(|_| LoraError::RecvError)?;
            }
        }
        Ok(())
    }

    async fn process(&mut self) -> Result<(), LoraError> {
        let uart = self.uart.as_ref().unwrap();

        let mut buf = [0; 1];

        let len = uart.read(&mut buf[..]).await?;
        for b in &buf[..len] {
            self.parse_buffer.write(*b).unwrap();
        }
        Ok(())
    }
}

impl<U, Q> Actor for Rak811Ingress<U, Q>
where
    U: UartReader,
    Q: Queue<T = RakResponse> + 'static,
{
    type Configuration = (Address<Q>, Address<U>);
    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.response_queue.replace(config.0);
        self.uart.replace(config.1);
    }

    fn on_start(mut self) -> Completion<Self> {
        Completion::defer(async move {
            log::info!("[{}] Starting RAK811 Ingress", ActorInfo::name());
            loop {
                if let Err(e) = self.process().await {
                    log::error!("Error reading data: {:?}", e);
                }

                if let Err(e) = self.digest().await {
                    log::error!("Error digesting data");
                }
            }
        })
    }
}

impl core::convert::From<UartError> for LoraError {
    fn from(error: UartError) -> Self {
        log::debug!("Convert from UART error {:?}", error);
        match error {
            UartError::TxInProgress
            | UartError::TxBufferTooSmall
            | UartError::TxBufferTooLong
            | UartError::Transmit => LoraError::SendError,
            UartError::RxInProgress
            | UartError::RxBufferTooSmall
            | UartError::RxBufferTooLong
            | UartError::Receive => LoraError::RecvError,
            _ => LoraError::OtherError,
        }
    }
}

impl core::convert::From<DriverError> for LoraError {
    fn from(error: DriverError) -> Self {
        log::debug!("Convert from {:?}", error);
        match error {
            DriverError::NotInitialized => LoraError::NotInitialized,
            DriverError::WriteError => LoraError::SendError,
            DriverError::ReadError => LoraError::RecvError,
            DriverError::OtherError | DriverError::UnexpectedResponse => LoraError::OtherError,
        }
    }
}
