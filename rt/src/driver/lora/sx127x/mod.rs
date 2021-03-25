use crate::{
    api::{lora::*, scheduler::*},
    domain::time::{
        duration::Milliseconds,
        rate::{Hertz, Rate},
    },
    hal::gpio::InterruptPin,
    prelude::*,
};
use core::cell::RefCell;
use cortex_m::interrupt::Nr;
use embedded_hal::{
    blocking::{
        delay::DelayMs,
        spi::{Transfer, Write},
    },
    digital::v2::{InputPin, OutputPin},
};

use lorawan_device::{
    radio, Device as LorawanDevice, Error as LorawanError, Event as LorawanEvent,
    Region as LorawanRegion, Response as LorawanResponse,
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
    actor: ActorContext<Sx127xActor<S, SPI, CS, RESET, BUSY, DELAY, E>>,
    interrupt: InterruptContext<Sx127xInterrupt<S, SPI, CS, RESET, BUSY, DELAY, READY, E>>,
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
            actor: ActorContext::new(Sx127xActor::new(busy, delay, get_random)?)
                .with_name("sx127x_actor"),
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
    type Primary = Sx127xActor<S, SPI, CS, RESET, BUSY, DELAY, E>;
    type Configuration = Address<S>;
    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        let actor = self.actor.mount((config, &self.state), supervisor);
        self.interrupt.mount(actor.clone(), supervisor);
        actor
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.actor.address()
    }
}

pub struct Sx127xActor<S, SPI, CS, RESET, BUSY, DELAY, E>
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
    me: Option<Address<Self>>,
    scheduler: Option<Address<S>>,
    busy: BUSY,
    delay: DELAY,
    get_random: fn() -> u32,
}

impl<S, SPI, CS, RESET, BUSY, DELAY, E> Sx127xActor<S, SPI, CS, RESET, BUSY, DELAY, E>
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
                        log::info!("New Session Request");
                    }
                    LorawanEvent::RadioEvent(e) => match e {
                        radio::Event::TxRequest(_, _) => (),
                        radio::Event::RxRequest(_) => (),
                        radio::Event::CancelRx => (),
                        radio::Event::PhyEvent(phy) => {
                            log::info!("Phy event");
                        }
                    },
                    LorawanEvent::TimeoutFired => (),
                    LorawanEvent::SendDataRequest(_e) => {
                        log::info!("SendData");
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
                    log::info!("TimeoutRequest: {:?}", ms);
                    self.scheduler.as_ref().unwrap().schedule(
                        Milliseconds(ms),
                        LorawanEvent::TimeoutFired,
                        self.me.as_ref().unwrap().clone(),
                    );
                }
                LorawanResponse::JoinSuccess => {
                    log::info!("Join Success: {:?}", lorawan.get_session_keys().unwrap());
                }
                LorawanResponse::ReadyToSend => {
                    log::info!("RxWindow expired but no ACK expected. Ready to Send");
                }
                LorawanResponse::DownlinkReceived(fcnt_down) => {
                    if let Some(downlink) = lorawan.take_data_downlink() {
                        let fhdr = downlink.fhdr();
                        let fopts = fhdr.fopts();
                        use lorawan_encoding::parser::{DataHeader, FRMPayload};

                        if let Ok(FRMPayload::Data(data)) = downlink.frm_payload() {
                            log::info!(
                                "Downlink received \t\t(FCntDown={}\tFRM: {:?})",
                                fcnt_down,
                                data,
                            );
                        } else {
                            log::info!("Downlink received \t\t(FcntDown={})", fcnt_down);
                        }

                        let mut mac_commands_len = 0;
                        for mac_command in fopts {
                            if mac_commands_len == 0 {
                                log::info!("\tFOpts: ");
                            }
                            log::info!("{:?},", mac_command);
                            mac_commands_len += 1;
                        }
                    }
                }
                LorawanResponse::NoAck => {
                    log::info!("RxWindow expired, expected ACK to confirmed uplink not received");
                }
                LorawanResponse::NoJoinAccept => {
                    log::info!("No Join Accept Received");
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
                    log::info!("No update");
                }
                LorawanResponse::UplinkSending(fcnt_up) => {
                    log::info!("Uplink with FCnt {}", fcnt_up);
                }
                LorawanResponse::JoinRequestSending => {
                    log::info!("Join Request Sending");
                }
            },
            Err(err) => match err {
                LorawanError::Radio(_) => log::info!("Radio"),
                LorawanError::Session(e) => log::info!("Session {:?}", e),
                LorawanError::NoSession(_) => log::info!("NoSession"),
            },
        }
    }
}

impl<S, SPI, CS, RESET, BUSY, DELAY, E> Actor for Sx127xActor<S, SPI, CS, RESET, BUSY, DELAY, E>
where
    S: Scheduler,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    BUSY: InputPin,
    DELAY: DelayMs<u8>,
{
    type Configuration = (Address<S>, &'static DriverState<SPI, CS, RESET, E>);

    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.scheduler.replace(config.0);
        self.state.replace(config.1);
        self.me.replace(me);
    }

    fn on_initialize(mut self) -> Completion<Self> {
        log::info!("Initializing");
        let state = self.state.as_ref().unwrap();
        match state.take() {
            State::Uninitialized(spi, cs, reset) => {
                log::info!("Initializing radio");
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
        log::info!("Started actor");
        Completion::immediate(self)
    }
}

impl<S, SPI, CS, RESET, BUSY, DELAY, E>
    RequestHandler<LorawanEvent<'static, Radio<SPI, CS, RESET, E>>>
    for Sx127xActor<S, SPI, CS, RESET, BUSY, DELAY, E>
where
    S: Scheduler,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    BUSY: InputPin,
    DELAY: DelayMs<u8>,
{
    type Response = ();
    fn on_request(
        mut self,
        message: LorawanEvent<'static, Radio<SPI, CS, RESET, E>>,
    ) -> Response<Self, Self::Response> {
        Response::defer(async move {
            self.process_event(message).await;
            (self, ())
        })
    }
}

impl<S, SPI, CS, RESET, BUSY, DELAY, E> LoraDriver
    for Sx127xActor<S, SPI, CS, RESET, BUSY, DELAY, E>
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
                let lorawan: LorawanDevice<Radio<SPI, CS, RESET, E>, Crypto> = LorawanDevice::new(
                    radio,
                    dev_eui.reverse().into(),
                    app_eui.reverse().into(),
                    app_key.clone().into(),
                    self.get_random,
                );
                log::info!("Device created, updating state");
                state.replace(State::Configured(lorawan));
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
}

pub struct Sx127xInterrupt<S, SPI, CS, RESET, BUSY, DELAY, READY, E>
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
    actor: Option<Address<Sx127xActor<S, SPI, CS, RESET, BUSY, DELAY, E>>>,
    ready: READY,
}

impl<S, SPI, CS, RESET, BUSY, DELAY, READY, E>
    Sx127xInterrupt<S, SPI, CS, RESET, BUSY, DELAY, READY, E>
where
    S: Scheduler,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    BUSY: InputPin,
    DELAY: DelayMs<u8>,
    READY: InterruptPin,
{
    pub fn new(ready: READY) -> Self {
        Self { ready, actor: None }
    }
}

impl<S, SPI, CS, RESET, BUSY, DELAY, READY, E> Actor
    for Sx127xInterrupt<S, SPI, CS, RESET, BUSY, DELAY, READY, E>
where
    S: Scheduler,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    BUSY: InputPin,
    DELAY: DelayMs<u8>,
    READY: InterruptPin,
{
    type Configuration = Address<Sx127xActor<S, SPI, CS, RESET, BUSY, DELAY, E>>;

    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.actor.replace(config);
    }
}

impl<S, SPI, CS, RESET, BUSY, DELAY, READY, E> Interrupt
    for Sx127xInterrupt<S, SPI, CS, RESET, BUSY, DELAY, READY, E>
where
    S: Scheduler,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    BUSY: InputPin,
    DELAY: DelayMs<u8>,
    READY: InterruptPin,
{
    fn on_interrupt(&mut self) {
        if self.ready.check_interrupt() {
            self.ready.clear_interrupt();
            self.actor
                .as_ref()
                .unwrap()
                .notify(LorawanEvent::RadioEvent(radio::Event::PhyEvent(
                    RadioPhyEvent::Irq,
                )));
        }
    }
}
