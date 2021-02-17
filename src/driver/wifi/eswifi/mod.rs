mod parser;
mod ready;

use crate::api::arbitrator::BusArbitrator;
use crate::api::delayer::Delayer;
use crate::api::spi::{ChipSelect, SpiBus, SpiError};
use crate::domain::time::duration::Milliseconds;
use crate::driver::spi::SpiController;
use crate::driver::wifi::eswifi::parser::JoinResponse;
use crate::driver::wifi::eswifi::ready::{AwaitReady, QueryReady};
use crate::driver::wifi::eswifi::ready::{EsWifiReady, EsWifiReadyPin};
use crate::hal::gpio::exti_pin::ExtiPin;
use crate::prelude::*;
use core::borrow::BorrowMut;
use core::cell::RefCell;
use core::fmt::Write;
use core::future::Future;
use core::marker::PhantomData;
use core::pin::Pin;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::{Context, Poll, Waker};
use cortex_m::interrupt::Nr;
use embedded_hal::digital::v2::{InputPin, OutputPin};
use heapless::{consts::*, ArrayLength, String};

#[derive(Debug)]
pub enum Join {
    Open,
    Wep {
        ssid: String<U32>,
        password: String<U32>,
    },
}

impl Join {
    pub(crate) fn validate(&self) -> Result<&Self, JoinError> {
        match self {
            Join::Open => Ok(self),
            Join::Wep { ssid, password } => {
                if ssid.len() > 32 {
                    Err(JoinError::InvalidSsid)
                } else if password.len() > 32 {
                    Err(JoinError::InvalidPassword)
                } else {
                    Ok(self)
                }
            }
            _ => Ok(self),
        }
    }
}

#[derive(Debug)]
pub enum JoinError {
    Unknown,
    InvalidSsid,
    InvalidPassword,
    UnableToAssociate,
}

pub struct EsWifi<SPI, T, CS, READY, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8> + 'static,
    T: Delayer + 'static,
    CS: OutputPin + 'static,
    READY: InputPin + ExtiPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
{
    controller: ActorContext<EsWifiController<SPI, T, CS, RESET, WAKEUP>>,
    ready: EsWifiReady<READY>,
}

impl<SPI, T, CS, READY, RESET, WAKEUP> EsWifi<SPI, T, CS, READY, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8>,
    T: Delayer + 'static,
    CS: OutputPin + 'static,
    READY: InputPin + ExtiPin + 'static,
    RESET: OutputPin + 'static,
    WAKEUP: OutputPin + 'static,
{
    pub fn new<READY_IRQ: Nr>(
        cs: CS,
        ready: READY,
        ready_irq: READY_IRQ,
        reset: RESET,
        wakeup: WAKEUP,
    ) -> Self {
        Self {
            controller: ActorContext::new(EsWifiController::new(cs, reset, wakeup))
                .with_name("es-wifi"),
            ready: EsWifiReady::new(ready, ready_irq),
        }
    }
}

impl<SPI, T, CS, READY, RESET, WAKEUP> Package for EsWifi<SPI, T, CS, READY, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8>,
    T: Delayer + 'static,
    CS: OutputPin,
    READY: InputPin + ExtiPin,
    RESET: OutputPin,
    WAKEUP: OutputPin,
{
    type Primary = EsWifiController<SPI, T, CS, RESET, WAKEUP>;
    type Configuration = (Address<BusArbitrator<SPI>>, Address<T>);

    fn mount(
        &'static self,
        config: Self::Configuration,
        supervisor: &mut Supervisor,
    ) -> Address<Self::Primary> {
        let ready_addr = self.ready.mount((), supervisor);
        let controller_addr = self
            .controller
            .mount((config.0, config.1, ready_addr), supervisor);
        controller_addr
    }

    fn primary(&'static self) -> Address<Self::Primary> {
        self.controller.address()
    }
}

enum State {
    Uninitialized,
    Ready,
}

pub struct EsWifiController<SPI, T, CS, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8> + 'static,
    T: Delayer + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin,
    WAKEUP: OutputPin,
{
    spi: Option<Address<BusArbitrator<SPI>>>,
    delayer: Option<Address<T>>,
    ready: Option<Address<EsWifiReadyPin>>,
    cs: ChipSelect<CS, T>,
    reset: RESET,
    wakeup: WAKEUP,
    state: State,
}

macro_rules! command {
    ($size:tt, $($arg:tt)*) => ({
        //let mut c = String::new();
        //c
        let mut c = String::<$size>::new();
        write!(c, $($arg)*);
        c.push_str("\r");
        c
    })
}

impl<SPI, T, CS, RESET, WAKEUP> EsWifiController<SPI, T, CS, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8> + 'static,
    T: Delayer + 'static,
    CS: OutputPin,
    RESET: OutputPin,
    WAKEUP: OutputPin,
{
    pub fn new(cs: CS, reset: RESET, wakeup: WAKEUP) -> Self {
        Self {
            spi: None,
            delayer: None,
            ready: None,
            cs: ChipSelect::new(cs, Milliseconds(100u32)),
            reset,
            wakeup,
            state: State::Uninitialized,
        }
    }

    async fn wakeup(&mut self) {
        self.wakeup.set_low();
        self.delayer.unwrap().delay(Milliseconds(50u32)).await;
        self.wakeup.set_high();
        self.delayer.unwrap().delay(Milliseconds(50u32)).await;
    }

    async fn reset(&mut self) {
        self.reset.set_low();
        self.delayer.unwrap().delay(Milliseconds(50u32)).await;
        self.reset.set_high();
        self.delayer.unwrap().delay(Milliseconds(50u32)).await;
    }

    async fn await_data_ready(&self) {
        self.ready.unwrap().request(AwaitReady {}).await
    }

    async fn is_data_ready(&self) -> bool {
        self.ready.unwrap().request(QueryReady {}).await
    }

    async fn start(mut self) -> Self {
        log::info!("[{}] start", ActorInfo::name());
        self.reset().await;
        self.wakeup().await;

        let mut response = [0 as u8; 16];
        let mut pos = 0;

        self.await_data_ready().await;
        {
            let mut spi = self.spi.unwrap().begin_transaction().await;
            let cs = self.cs.select().await;

            loop {
                if !self.is_data_ready().await {
                    break;
                }
                if pos >= response.len() {
                    break;
                }
                let mut chunk = [0x0A, 0x0A];
                spi.spi_transfer(&mut chunk).await;
                // reverse order going from 16 -> 2*8 bits
                if chunk[1] != NAK {
                    response[pos] = chunk[1];
                    pos += 1;
                }
                if chunk[0] != NAK {
                    response[pos] = chunk[0];
                    pos += 1;
                }
            }
        }

        let needle = &[b'\r', b'\n', b'>', b' '];

        if !response[0..pos].starts_with(needle) {
            log::info!(
                "[{}] failed to initialize {:?}",
                ActorInfo::name(),
                &response[0..pos]
            );
        } else {
            // disable verbosity
            self.send_string(&command!(U8, "MT=1"), &mut response).await;
            self.state = State::Ready;
            log::info!("[{}] eS-WiFi adapter is ready", ActorInfo::name());
        }

        self.join(Join::Wep {
            ssid: "oddly".into(),
            password: "scarletbegonias".into(),
        })
        .await;

        self
    }

    async fn send_string<'a, N: ArrayLength<u8>>(
        &mut self,
        command: &String<N>,
        response: &'a mut [u8],
    ) -> Result<&'a [u8], SpiError> {
        self.send(command.as_bytes(), response).await
    }

    async fn send<'a>(
        &mut self,
        command: &[u8],
        response: &'a mut [u8],
    ) -> Result<&'a [u8], SpiError> {
        //log::info!("send {:?}", core::str::from_utf8(command).unwrap());

        //log::info!("await ready");
        self.await_data_ready().await;
        //log::info!("await ready done");
        {
            //log::info!("obtain spi");
            let mut spi = self.spi.unwrap().begin_transaction().await;
            //log::info!("obtain spi done");
            //log::info!("set cs");
            let _cs = self.cs.select().await;
            //log::info!("set cs done");

            for chunk in command.chunks(2) {
                let mut xfer: [u8; 2] = [0; 2];
                xfer[1] = chunk[0];
                if chunk.len() == 2 {
                    xfer[0] = chunk[1]
                } else {
                    xfer[0] = 0x0A
                }

                //log::info!("do xfer");
                spi.spi_transfer(&mut xfer).await?;

                self.delayer.unwrap().delay(Milliseconds(100u32)).await;
            }
            //log::info!("complete send xfer done");
        }
        self.receive(response).await
    }

    async fn receive<'a>(&mut self, response: &'a mut [u8]) -> Result<&'a [u8], SpiError> {
        self.await_data_ready().await;
        //log::info!("ready go");
        let mut pos = 0;

        let mut spi = self.spi.unwrap().begin_transaction().await;
        //log::info!("b");
        let _cs = self.cs.select().await;
        //log::info!("c");

        while self.is_data_ready().await {
            //log::info!("d");
            let mut xfer: [u8; 2] = [0x0A, 0x0A];
            let result = spi.spi_transfer(&mut xfer).await?;
            response[pos] = xfer[1];
            pos += 1;
            if xfer[0] != 0x15 {
                response[pos] = xfer[0];
                pos += 1;
            }
        }
        //log::info!("response complete");
        //log::info!(
        //"response {}",
        //core::str::from_utf8(&response[0..pos]).unwrap()
        //);
        Ok(&mut response[0..pos])
    }

    pub(crate) async fn join(&mut self, join_info: Join) -> Result<(), JoinError> {
        match join_info {
            Join::Open => Ok(()),
            Join::Wep { ssid, password } => {
                let mut response = [0u8; 1024];

                self.send_string(&command!(U36, "CB=2"), &mut response)
                    .await
                    .map_err(|_| JoinError::InvalidSsid)?;

                self.send_string(&command!(U36, "C1={}", ssid), &mut response)
                    .await
                    .map_err(|_| JoinError::InvalidSsid)?;

                self.send_string(&command!(U72, "C2={}", password), &mut response)
                    .await
                    .map_err(|_| JoinError::InvalidPassword)?;

                self.send_string(&command!(U8, "C3=4"), &mut response)
                    .await
                    .map_err(|_| JoinError::Unknown)?;

                let response = self
                    .send_string(&command!(U4, "C0"), &mut response)
                    .await
                    .map_err(|_| JoinError::Unknown)?;

                //log::info!("[[{}]]", core::str::from_utf8(&response).unwrap());

                let parse_result = parser::join_response(&response);

                log::info!("response for JOIN {:?}", parse_result);

                match parse_result {
                    Ok((_, response)) => match response {
                        JoinResponse::Ok => Ok(()),
                        JoinResponse::JoinError => Err(JoinError::UnableToAssociate),
                    },
                    Err(_) => {
                        log::info!("{:?}", &response);
                        Err(JoinError::UnableToAssociate)
                    }
                }
            }
        }
    }
}

const NAK: u8 = 0x15;

impl<SPI, T, CS, RESET, WAKEUP> Actor for EsWifiController<SPI, T, CS, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8>,
    T: Delayer + 'static,
    CS: OutputPin,
    RESET: OutputPin,
    WAKEUP: OutputPin,
{
    type Configuration = (
        Address<BusArbitrator<SPI>>,
        Address<T>,
        Address<EsWifiReadyPin>,
    );

    fn on_mount(&mut self, address: Address<Self>, config: Self::Configuration)
    where
        Self: Sized,
    {
        self.spi.replace(config.0);
        self.delayer.replace(config.1);
        self.ready.replace(config.2);
        self.cs.set_delayer(config.1);
    }

    fn on_start(mut self) -> Completion<Self>
    where
        Self: 'static,
    {
        Completion::defer(self.start())
    }
}

impl<SPI, T, CS, RESET, WAKEUP> RequestHandler<Join> for EsWifiController<SPI, T, CS, RESET, WAKEUP>
where
    SPI: SpiBus<Word = u8>,
    T: Delayer + 'static,
    CS: OutputPin,
    RESET: OutputPin,
    WAKEUP: OutputPin,
{
    type Response = Result<(), JoinError>;

    fn on_request(mut self, message: Join) -> Response<Self, Self::Response> {
        Response::defer(async move {
            let result = self.join(message).await;
            (self, result)
        })
    }
}

impl<SPI, T, CS, RESET, WAKEUP> Address<EsWifiController<SPI, T, CS, RESET, WAKEUP>>
where
    SPI: SpiBus<Word = u8>,
    T: Delayer + 'static,
    CS: OutputPin,
    RESET: OutputPin,
    WAKEUP: OutputPin,
{
    // TODO a wifi trait
    pub async fn join(&self, join: Join) -> Result<(), JoinError> {
        self.request(join).await
    }
}
