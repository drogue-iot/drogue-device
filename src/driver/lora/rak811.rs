use crate::api::{
    delayer::*,
    lora::*,
    scheduler::*,
    uart::{Error as UartError, UartReader, UartWriter},
};
use crate::domain::time::duration::Milliseconds;
use crate::handler::Response;
use crate::prelude::*;

use core::cell::{RefCell, UnsafeCell};

use drogue_rak811::{
    Buffer, Command, ConfigOption, DriverError, EventCode, Response as RakResponse,
};
use embedded_hal::digital::v2::OutputPin;
use heapless::{
    consts,
    spsc::{Consumer, Producer, Queue},
    String,
};

pub struct Rak811Actor<U, T, RST>
where
    U: UartWriter + 'static,
    T: Scheduler + Delayer + 'static,
    RST: OutputPin + 'static,
{
    uart: Option<Address<U>>,
    timer: Option<Address<T>>,
    command_buffer: String<consts::U128>,
    config: LoraConfig,
    rst: RST,
    rxc: Option<RefCell<Consumer<'static, RakResponse, consts::U8>>>,
}
pub struct Rak811Ingress<U, T>
where
    U: UartReader + 'static,
    T: Scheduler + Delayer + 'static,
{
    uart: Option<Address<U>>,
    timer: Option<Address<T>>,
    parse_buffer: Buffer,
    rxp: Option<RefCell<Producer<'static, RakResponse, consts::U8>>>,
}

pub struct Rak811<U, T, RST>
where
    U: UartReader + UartWriter + 'static,
    T: Scheduler + Delayer + 'static,
    RST: OutputPin + 'static,
{
    actor: ActorContext<Rak811Actor<U, T, RST>>,
    ingress: ActorContext<Rak811Ingress<U, T>>,
    rxq: UnsafeCell<Queue<RakResponse, consts::U8>>,
}

impl<U, T, RST> Package for Rak811<U, T, RST>
where
    U: UartReader + UartWriter + 'static,
    T: Scheduler + Delayer + 'static,
    RST: OutputPin + 'static,
{
    type Primary = Rak811Actor<U, T, RST>;
    type Configuration = (Address<U>, Address<T>);
    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary>
    where
        Self: 'static,
    {
        /*
        let mut queue = self.rxq.borrow_mut();
        let (prod, cons): (
            Producer<'static, RakResponse, consts::U8>,
            Consumer<'static, RakResponse, consts::U8>,
        ) = queue.split();*/
        let (prod, cons) = unsafe { (&mut *self.rxq.get()).split() };
        self.ingress.mount((prod, config.0, config.1), supervisor);
        let addr = self.actor.mount((cons, config.0, config.1), supervisor);

        addr
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.actor.address()
    }
}

impl<U, T, RST> Rak811<U, T, RST>
where
    U: UartReader + UartWriter + 'static,
    T: Scheduler + Delayer + 'static,
    RST: OutputPin,
{
    pub fn new(rst: RST) -> Self {
        Self {
            actor: ActorContext::new(Rak811Actor::new(rst)).with_name("rak811_actor"),
            ingress: ActorContext::new(Rak811Ingress::new()).with_name("rak811_ingress"),
            rxq: UnsafeCell::new(Queue::new()),
        }
    }
}

impl<U, T, RST> Rak811Actor<U, T, RST>
where
    U: UartWriter,
    T: Scheduler + Delayer + 'static,
    RST: OutputPin,
{
    pub fn new(rst: RST) -> Self {
        Self {
            uart: None,
            timer: None,
            command_buffer: String::new(),
            config: LoraConfig::new(),
            rst,
            rxc: None,
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
        loop {
            // Run processing to increase likelyhood we have something to parse.
            if let Some(response) = self.rxc.as_ref().unwrap().borrow_mut().dequeue() {
                return Ok(response);
            }
            self.timer.as_ref().unwrap().delay(Milliseconds(1000)).await;
        }
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

impl<U, T, RST> Actor for Rak811Actor<U, T, RST>
where
    U: UartWriter,
    T: Scheduler + Delayer + 'static,
    RST: OutputPin,
{
    type Configuration = (
        Consumer<'static, RakResponse, consts::U8>,
        Address<U>,
        Address<T>,
    );
    fn on_mount(&mut self, _: Address<Self>, config: Self::Configuration) {
        self.rxc.replace(RefCell::new(config.0));
        self.uart.replace(config.1);
        self.timer.replace(config.2);
    }

    fn on_initialize(mut self) -> Completion<Self> {
        Completion::defer(async move {
            log::debug!("RAK811 LoRa module initializing");
            self.rst.set_high().ok();
            self.timer.as_ref().unwrap().delay(Milliseconds(50)).await;
            self.rst.set_low().ok();
            let response = self.recv_response().await;
            match response {
                Ok(RakResponse::Initialized(band)) => {
                    self.config.band.replace(band);
                    log::info!("RAK811 driver initialized with band {:?}", band);
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

impl<U, T, RST> LoraDriver for Rak811Actor<U, T, RST>
where
    U: UartWriter,
    T: Scheduler + Delayer + 'static,
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
                            log::info!("Received response: {:?}", r);
                            Err(LoraError::OtherError)
                        }
                    }
                }
                r => {
                    log::info!("Received response: {:?}", r);
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
}

impl<U, T> Rak811Ingress<U, T>
where
    U: UartReader,
    T: Scheduler + Delayer + 'static,
{
    pub fn new() -> Self {
        Self {
            uart: None,
            timer: None,
            parse_buffer: Buffer::new(),
            rxp: None,
        }
    }

    fn digest(&mut self) -> Result<(), LoraError> {
        let result = self.parse_buffer.parse();
        if let Ok(response) = result {
            if !matches!(response, RakResponse::None) {
                log::info!("Got response: {:?}", response);
                self.rxp
                    .as_ref()
                    .unwrap()
                    .borrow_mut()
                    .enqueue(response)
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

impl<U, T> Actor for Rak811Ingress<U, T>
where
    U: UartReader,
    T: Scheduler + Delayer + 'static,
{
    type Configuration = (
        Producer<'static, RakResponse, consts::U8>,
        Address<U>,
        Address<T>,
    );
    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.rxp.replace(RefCell::new(config.0));
        self.uart.replace(config.1);
        self.timer.replace(config.2);
    }

    fn on_start(mut self) -> Completion<Self> {
        Completion::defer(async move {
            log::info!("Starting RAK811 Ingress");
            loop {
                if let Err(e) = self.process().await {
                    log::error!("Error reading data: {:?}", e);
                }

                if let Err(e) = self.digest() {
                    log::error!("Error digesting data");
                }
            }
        })
    }
}

impl core::convert::From<UartError> for LoraError {
    fn from(error: UartError) -> Self {
        log::info!("Convert from UART error {:?}", error);
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
        log::info!("Convert from {:?}", error);
        match error {
            DriverError::NotInitialized => LoraError::NotInitialized,
            DriverError::WriteError => LoraError::SendError,
            DriverError::ReadError => LoraError::RecvError,
            DriverError::OtherError | DriverError::UnexpectedResponse => LoraError::OtherError,
        }
    }
}
