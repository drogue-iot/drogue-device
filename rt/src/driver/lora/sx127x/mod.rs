use crate::{
    api::{lora::*, scheduler::*},
    domain::time::{
        duration::Milliseconds,
        rate::{Hertz, Rate},
    },
    hal::gpio::InterruptPin,
    prelude::*,
    synchronization::Signal,
};
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use cortex_m::interrupt::Nr;
use embedded_hal::{
    blocking::{
        delay::DelayMs,
        spi::{Transfer, Write},
    },
    digital::v2::{InputPin, OutputPin},
};
use heapless::{consts, Vec};

use lorawan_device::{
    radio, region, Device as LorawanDevice, Error as LorawanError, Event as LorawanEvent,
    Region as LorawanRegion, Response as LorawanResponse, Timings as RadioTimings,
};
use lorawan_encoding::default_crypto::DefaultFactory as Crypto;

mod sx127x_lora;
mod sx127x_radio;

use sx127x_radio::{RadioPhyEvent, Sx127xRadio as Radio};

pub struct DriverState<SPI, CS, RESET, E>
where
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    E: 'static,
{
    inner: RefCell<Option<State<SPI, CS, RESET, E>>>,
}

impl<SPI, CS, RESET, E> DriverState<SPI, CS, RESET, E>
where
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    fn new(spi: SPI, cs: CS, reset: RESET) -> Self {
        Self {
            inner: RefCell::new(Some(State::Uninitialized(spi, cs, reset))),
        }
    }

    fn replace(&self, state: State<SPI, CS, RESET, E>) {
        self.inner.borrow_mut().replace(state);
    }

    fn take(&self) -> State<SPI, CS, RESET, E> {
        self.inner.borrow_mut().take().unwrap()
    }
}

enum State<SPI, CS, RESET, E>
where
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    E: 'static,
{
    Uninitialized(SPI, CS, RESET),
    Initialized(Radio<SPI, CS, RESET, E>),
    Configured(LorawanDevice<Radio<SPI, CS, RESET, E>, Crypto>),
}

pub type ControllerResponse = Result<Option<Vec<u8, consts::U255>>, LoraError>;

pub struct Sx127x<S, SPI, CS, RESET, BUSY, DELAY, READY, E>
where
    S: Scheduler + 'static,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    BUSY: InputPin + 'static,
    DELAY: DelayMs<u8> + 'static,
    READY: InterruptPin + 'static,
    E: 'static,
{
    state: DriverState<SPI, CS, RESET, E>,
    response: Signal<ControllerResponse>,
    actor: ActorContext<Sx127xActor<Sx127xController<S, SPI, CS, RESET, BUSY, DELAY, E>>>,
    controller: ActorContext<Sx127xController<S, SPI, CS, RESET, BUSY, DELAY, E>>,
    interrupt: InterruptContext<
        Sx127xInterrupt<
            Sx127xController<S, SPI, CS, RESET, BUSY, DELAY, E>,
            Radio<SPI, CS, RESET, E>,
            READY,
        >,
    >,
}

impl<S, SPI, CS, RESET, BUSY, DELAY, READY, E> Sx127x<S, SPI, CS, RESET, BUSY, DELAY, READY, E>
where
    S: Scheduler,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E>,
    CS: OutputPin,
    RESET: OutputPin,
    BUSY: InputPin,
    DELAY: DelayMs<u8>,
    READY: InterruptPin,
{
    pub fn new<IRQ>(
        spi: SPI,
        cs: CS,
        reset: RESET,
        busy: BUSY,
        delay: DELAY,
        ready: READY,
        irq: IRQ,
        get_random: fn() -> u32,
    ) -> Result<Self, LoraError>
    where
        IRQ: Nr,
    {
        Ok(Self {
            state: DriverState::new(spi, cs, reset),
            response: Signal::new(),
            actor: ActorContext::new(Sx127xActor::new()).with_name("sx127x_actor"),
            controller: ActorContext::new(Sx127xController::new(busy, delay, get_random)?)
                .with_name("sx127x_controller"),
            interrupt: InterruptContext::new(Sx127xInterrupt::new(ready), irq)
                .with_name("sx127x_interrupt"),
        })
    }
}

impl<S, SPI, CS, RESET, BUSY, DELAY, READY, E> Package
    for Sx127x<S, SPI, CS, RESET, BUSY, DELAY, READY, E>
where
    S: Scheduler,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    BUSY: InputPin,
    DELAY: DelayMs<u8>,
    READY: InterruptPin,
{
    type Primary = Sx127xActor<Sx127xController<S, SPI, CS, RESET, BUSY, DELAY, E>>;
    type Configuration = Address<S>;
    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        let controller = self
            .controller
            .mount((config, &self.state, &self.response), supervisor);
        let actor = self.actor.mount((controller, &self.response), supervisor);
        self.interrupt.mount(controller, supervisor);
        actor
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.actor.address()
    }
}

pub struct Sx127xActor<H>
where
    H: LoraDriver + 'static,
{
    controller: Option<Address<H>>,
    response: Option<&'static Signal<ControllerResponse>>,
}

impl<H> Sx127xActor<H>
where
    H: LoraDriver,
{
    pub fn new() -> Self {
        Self {
            controller: None,
            response: None,
        }
    }
}

impl<H> LoraDriver for Sx127xActor<H>
where
    H: LoraDriver,
{
    fn configure<'a>(mut self, message: Configure<'a>) -> Response<Self, Result<(), LoraError>> {
        // log_stack("Sx127xActor configure");
        unsafe {
            Response::defer_unchecked(async move {
                let response = self.response.as_ref().unwrap();
                response.reset();
                self.controller.as_ref().unwrap().configure(message.0).await;
                let ret = DriverResponse::new(response).await.map(|r| ());
                (self, ret)
            })
        }
    }

    fn reset(self, message: Reset) -> Response<Self, Result<(), LoraError>> {
        Response::immediate(self, Err(LoraError::OtherError))
    }

    fn join(mut self, message: Join) -> Response<Self, Result<(), LoraError>> {
        Response::defer(async move {
            let response = self.response.as_ref().unwrap();
            response.reset();
            self.controller.as_ref().unwrap().join(message.0).await;
            let ret = DriverResponse::new(response).await.map(|r| ());
            (self, ret)
        })
    }

    fn send<'a>(self, message: Send<'a>) -> Response<Self, Result<(), LoraError>> {
        unsafe {
            Response::defer_unchecked(async move {
                let response = self.response.as_ref().unwrap();
                response.reset();
                self.controller
                    .as_ref()
                    .unwrap()
                    .request_panicking(message)
                    .await;
                match DriverResponse::new(response).await {
                    Ok(_) => (self, Ok(())),
                    Err(e) => (self, Err(e)),
                }
            })
        }
    }

    fn send_recv<'a>(self, message: SendRecv<'a>) -> Response<Self, Result<usize, LoraError>> {
        unsafe {
            Response::defer_unchecked(async move {
                let response = self.response.as_ref().unwrap();
                response.reset();
                let mut rx_buf = message.3;
                self.controller
                    .as_ref()
                    .unwrap()
                    .send(message.0, message.1, message.2)
                    .await;
                match DriverResponse::new(response).await {
                    Ok(Some(data)) => {
                        if rx_buf.len() < data.len() {
                            log::warn!("Receive buffer is too small!");
                            (self, Err(LoraError::RecvBufferTooSmall))
                        } else {
                            rx_buf[..data.len()].copy_from_slice(&data[..data.len()]);
                            (self, Ok(data.len()))
                        }
                    }
                    Ok(None) => (self, Ok(0)),
                    Err(e) => (self, Err(e)),
                }
            })
        }
    }
}

impl<H> Actor for Sx127xActor<H>
where
    H: LoraDriver,
{
    type Configuration = (Address<H>, &'static Signal<ControllerResponse>);

    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.controller.replace(config.0);
        self.response.replace(config.1);
    }
}

pub struct Sx127xController<S, SPI, CS, RESET, BUSY, DELAY, E>
where
    S: Scheduler + 'static,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    BUSY: InputPin + 'static,
    DELAY: DelayMs<u8> + 'static,
    E: 'static,
{
    state: Option<&'static DriverState<SPI, CS, RESET, E>>,
    response: Option<&'static Signal<ControllerResponse>>,
    me: Option<Address<Self>>,
    scheduler: Option<Address<S>>,
    busy: BUSY,
    delay: DELAY,
    get_random: fn() -> u32,
}

impl<S, SPI, CS, RESET, BUSY, DELAY, E> Sx127xController<S, SPI, CS, RESET, BUSY, DELAY, E>
where
    S: Scheduler + 'static,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    BUSY: InputPin + 'static,
    DELAY: DelayMs<u8> + 'static,
{
    pub fn new(busy: BUSY, delay: DELAY, get_random: fn() -> u32) -> Result<Self, LoraError> {
        Ok(Self {
            state: None,
            delay,
            me: None,
            scheduler: None,
            response: None,
            busy,
            get_random,
        })
    }

    async fn process_event(&mut self, event: LorawanEvent<'static, Radio<SPI, CS, RESET, E>>) {
        let state = self.state.as_ref().unwrap();
        match state.take() {
            State::Configured(lorawan) => {
                match &event {
                    LorawanEvent::NewSessionRequest => {
                        log::trace!("New Session Request");
                    }
                    LorawanEvent::RadioEvent(e) => match e {
                        radio::Event::TxRequest(_, _) => (),
                        radio::Event::RxRequest(_) => (),
                        radio::Event::CancelRx => (),
                        radio::Event::PhyEvent(phy) => {
                            // log::info!("Phy event");
                        }
                    },
                    LorawanEvent::TimeoutFired => (),
                    LorawanEvent::SendDataRequest(_e) => {
                        log::trace!("SendData");
                    }
                }
                // log_stack("Handling event");
                let (mut new_state, response) = lorawan.handle_event(event);
                // log::info!("Event handled");
                self.process_response(&mut new_state, response);
                state.replace(State::Configured(new_state));
            }
            s => {
                log::info!("Not yet configured, event processing skipped");
                state.replace(s);
            }
        }
    }

    fn process_response(
        &self,
        lorawan: &mut LorawanDevice<Radio<SPI, CS, RESET, E>, Crypto>,
        response: Result<LorawanResponse, LorawanError<Radio<SPI, CS, RESET, E>>>,
    ) {
        match response {
            Ok(response) => match response {
                LorawanResponse::TimeoutRequest(ms) => {
                    log::trace!("TimeoutRequest: {:?}", ms);
                    self.scheduler.as_ref().unwrap().schedule(
                        Milliseconds(ms),
                        LorawanEvent::TimeoutFired,
                        self.me.as_ref().unwrap().clone(),
                    );
                }
                LorawanResponse::JoinSuccess => {
                    log::trace!("Join Success: {:?}", lorawan.get_session_keys().unwrap());
                    self.response.as_ref().unwrap().signal(Ok(None));
                }
                LorawanResponse::ReadyToSend => {
                    log::trace!("RxWindow expired but no ACK expected. Ready to Send");
                }
                LorawanResponse::DownlinkReceived(fcnt_down) => {
                    if let Some(downlink) = lorawan.take_data_downlink() {
                        let fhdr = downlink.fhdr();
                        let fopts = fhdr.fopts();
                        use lorawan_encoding::parser::{DataHeader, FRMPayload};

                        if let Ok(FRMPayload::Data(data)) = downlink.frm_payload() {
                            log::trace!(
                                "Downlink received \t\t(FCntDown={}\tFRM: {:?})",
                                fcnt_down,
                                data,
                            );
                            let mut v = Vec::new();
                            v.extend_from_slice(data);
                            self.response.as_ref().unwrap().signal(Ok(Some(v)));
                        } else {
                            self.response.as_ref().unwrap().signal(Ok(None));
                            log::trace!("Downlink received \t\t(FcntDown={})", fcnt_down);
                        }

                        let mut mac_commands_len = 0;
                        for mac_command in fopts {
                            if mac_commands_len == 0 {
                                log::trace!("\tFOpts: ");
                            }
                            log::trace!("{:?},", mac_command);
                            mac_commands_len += 1;
                        }
                    }
                }
                LorawanResponse::NoAck => {
                    log::trace!("RxWindow expired, expected ACK to confirmed uplink not received");
                    self.response.as_ref().unwrap().signal(Ok(None));
                }
                LorawanResponse::NoJoinAccept => {
                    log::info!("No Join Accept Received. Retrying.");
                    self.me
                        .as_ref()
                        .unwrap()
                        .notify(LorawanEvent::NewSessionRequest);
                }
                LorawanResponse::SessionExpired => {
                    log::info!("SessionExpired. Created new Session");
                    self.me
                        .as_ref()
                        .unwrap()
                        .notify(LorawanEvent::NewSessionRequest);
                }
                LorawanResponse::NoUpdate => {
                    // log::info!("No update");
                }
                LorawanResponse::UplinkSending(fcnt_up) => {
                    log::trace!("Uplink with FCnt {}", fcnt_up);
                }
                LorawanResponse::JoinRequestSending => {
                    log::trace!("Join Request Sending");
                }
            },
            Err(err) => match err {
                LorawanError::Radio(_) => log::error!("Radio error"),
                LorawanError::Session(e) => log::error!("Session error {:?}", e),
                LorawanError::NoSession(_) => log::error!("NoSession error"),
            },
        }
    }
}

impl<S, SPI, CS, RESET, BUSY, DELAY, E> Actor
    for Sx127xController<S, SPI, CS, RESET, BUSY, DELAY, E>
where
    S: Scheduler,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    BUSY: InputPin,
    DELAY: DelayMs<u8>,
{
    type Configuration = (
        Address<S>,
        &'static DriverState<SPI, CS, RESET, E>,
        &'static Signal<ControllerResponse>,
    );

    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.scheduler.replace(config.0);
        self.state.replace(config.1);
        self.response.replace(config.2);
        self.me.replace(me);
    }

    fn on_initialize(mut self) -> Completion<Self> {
        let state = self.state.as_ref().unwrap();
        match state.take() {
            State::Uninitialized(spi, cs, reset) => {
                match Radio::new(spi, cs, reset, &mut self.delay) {
                    Ok(radio) => {
                        state.replace(State::Initialized(radio));
                    }
                    Err(e) => {
                        log::error!("Error initializing driver: {:?}", e);
                        // TODO: Figure out a way to keep configuration when failed
                    }
                }
            }
            other => {
                log::info!("Driver already initialized, skipping");
                state.replace(other);
            }
        }
        Completion::immediate(self)
    }

    fn on_start(mut self) -> Completion<Self> {
        Completion::immediate(self)
    }
}

impl<S, SPI, CS, RESET, BUSY, DELAY, E> LoraDriver
    for Sx127xController<S, SPI, CS, RESET, BUSY, DELAY, E>
where
    S: Scheduler,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    BUSY: InputPin,
    DELAY: DelayMs<u8>,
{
    fn configure<'a>(mut self, message: Configure<'a>) -> Response<Self, Result<(), LoraError>> {
        // log_stack("Sx127xActor configure");
        let state = self.state.as_ref().unwrap();
        match state.take() {
            State::Initialized(radio) => {
                //log::info!("Configuring radio");
                let config = message.0;
                let dev_eui = config.device_eui.as_ref().expect("device EUI must be set");
                let app_eui = config.app_eui.as_ref().expect("app EUI must be set");
                let app_key = config.app_key.as_ref().expect("app KEY must be set");
                //log::info!("Creating device");
                let mut lorawan: LorawanDevice<Radio<SPI, CS, RESET, E>, Crypto> =
                    LorawanDevice::new(
                        region::EU868::default().into(), // TODO: Make configurable
                        radio,
                        dev_eui.reverse().into(),
                        app_eui.reverse().into(),
                        app_key.clone().into(),
                        self.get_random,
                    );
                lorawan.set_datarate(5); // Use lower datarate that seems more stable
                state.replace(State::Configured(lorawan));
                self.response.as_ref().unwrap().signal(Ok(None));
                Response::immediate(self, Ok(()))
            }
            other => {
                //log::info!("Driver not yet initialized, ignoring configuration");
                state.replace(other);
                Response::immediate(self, Err(LoraError::OtherError))
            }
        }
    }

    fn reset(self, message: Reset) -> Response<Self, Result<(), LoraError>> {
        Response::immediate(self, Err(LoraError::OtherError))
    }

    fn join(mut self, message: Join) -> Response<Self, Result<(), LoraError>> {
        Response::defer(async move {
            self.process_event(LorawanEvent::NewSessionRequest).await;
            (self, Ok(()))
        })
    }

    fn send<'a>(self, message: Send<'a>) -> Response<Self, Result<(), LoraError>> {
        unsafe {
            Response::defer_unchecked(async move {
                let state = self.state.as_ref().unwrap();
                match state.take() {
                    State::Configured(lorawan) => {
                        let ready_to_send = lorawan.ready_to_send_data();
                        state.replace(if ready_to_send {
                            let (mut new_state, response) = lorawan.send(
                                message.2,
                                message.1,
                                match message.0 {
                                    QoS::Confirmed => true,
                                    QoS::Unconfirmed => false,
                                },
                            );
                            self.process_response(&mut new_state, response);
                            State::Configured(new_state)
                        } else {
                            State::Configured(lorawan)
                        });
                        (self, Ok(()))
                    }
                    other => {
                        //log::info!("Driver not yet initialized, ignoring configuration");
                        state.replace(other);
                        (self, Err(LoraError::OtherError))
                    }
                }
            })
        }
    }

    fn send_recv<'a>(self, message: SendRecv<'a>) -> Response<Self, Result<usize, LoraError>> {
        Response::immediate(self, Err(LoraError::NotImplemented))
    }
}

impl<S, SPI, CS, RESET, BUSY, DELAY, E>
    NotifyHandler<LorawanEvent<'static, Radio<SPI, CS, RESET, E>>>
    for Sx127xController<S, SPI, CS, RESET, BUSY, DELAY, E>
where
    S: Scheduler,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    BUSY: InputPin,
    DELAY: DelayMs<u8>,
{
    fn on_notify(
        mut self,
        message: LorawanEvent<'static, Radio<SPI, CS, RESET, E>>,
    ) -> Completion<Self> {
        Completion::defer(async move {
            self.process_event(message).await;
            self
        })
    }
}

pub struct Sx127xInterrupt<H, RADIO, READY>
where
    RADIO: radio::PhyRxTx + RadioTimings,
    H: NotifyHandler<LorawanEvent<'static, RADIO>> + 'static,
    READY: InterruptPin + 'static,
{
    controller: Option<Address<H>>,
    ready: READY,
    _phantom: core::marker::PhantomData<RADIO>,
}

impl<H, RADIO, READY> Sx127xInterrupt<H, RADIO, READY>
where
    RADIO: radio::PhyRxTx + RadioTimings,
    H: NotifyHandler<LorawanEvent<'static, RADIO>>,
    READY: InterruptPin + 'static,
{
    pub fn new(ready: READY) -> Self {
        Self {
            ready,
            controller: None,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<H, RADIO, READY> Actor for Sx127xInterrupt<H, RADIO, READY>
where
    RADIO: radio::PhyRxTx + RadioTimings,
    H: NotifyHandler<LorawanEvent<'static, RADIO>>,
    READY: InterruptPin + 'static,
{
    type Configuration = Address<H>;

    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.controller.replace(config);
    }
}

impl<H, RADIO, READY> Interrupt for Sx127xInterrupt<H, RADIO, READY>
where
    RADIO: radio::PhyRxTx<PhyEvent = RadioPhyEvent> + RadioTimings + 'static,
    H: NotifyHandler<LorawanEvent<'static, RADIO>>,
    READY: InterruptPin + 'static,
{
    fn on_interrupt(&mut self) {
        if self.ready.check_interrupt() {
            self.ready.clear_interrupt();
            self.controller
                .as_ref()
                .unwrap()
                .notify(LorawanEvent::RadioEvent(radio::Event::PhyEvent(
                    RadioPhyEvent::Irq,
                )));
        }
    }
}

struct DriverResponse {
    signal: &'static Signal<ControllerResponse>,
}

impl DriverResponse {
    pub fn new(signal: &'static Signal<ControllerResponse>) -> Self {
        Self { signal }
    }
}

impl core::future::Future for DriverResponse {
    type Output = ControllerResponse;

    fn poll(mut self: core::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.signal.poll_wait(cx)
    }
}
