use super::AdapterActor;
use crate::drivers::wifi::esp8266::*;
use crate::package::*;
use crate::{Actor, ActorContext, ActorSpawner, Address, Inbox};
use core::{
    cell::{RefCell, UnsafeCell},
    future::Future,
};
use embedded_hal::digital::v2::OutputPin;
use embedded_hal_async::serial::{Read, Write};

pub enum State<TX, RX, ENABLE, RESET>
where
    TX: Write + 'static,
    RX: Read + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    New(TX, RX, ENABLE, RESET),
    Initialized,
}

pub struct Esp8266Wifi<TX, RX, ENABLE, RESET>
where
    TX: Write + 'static,
    RX: Read + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    driver: UnsafeCell<Esp8266Driver>,
    state: RefCell<Option<State<TX, RX, ENABLE, RESET>>>,
    wifi: ActorContext<AdapterActor<Esp8266Controller<'static, TX>>, 4>,
    modem: ActorContext<ModemActor<'static, RX, ENABLE, RESET>>,
}

impl<TX, RX, ENABLE, RESET> Esp8266Wifi<TX, RX, ENABLE, RESET>
where
    TX: Write + 'static,
    RX: Read + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    pub fn new(tx: TX, rx: RX, enable: ENABLE, reset: RESET) -> Self {
        Self {
            driver: UnsafeCell::new(Esp8266Driver::new()),
            state: RefCell::new(Some(State::New(tx, rx, enable, reset))),
            wifi: ActorContext::new(),
            modem: ActorContext::new(),
        }
    }
}

impl<TX, RX, ENABLE, RESET> Package for Esp8266Wifi<TX, RX, ENABLE, RESET>
where
    TX: Write + 'static,
    RX: Read + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    type Primary = AdapterActor<Esp8266Controller<'static, TX>>;

    fn mount<S: ActorSpawner>(
        &'static self,
        _: Self::Configuration,
        spawner: S,
    ) -> Address<Self::Primary> {
        if let Some(State::New(tx, rx, enable, reset)) = self.state.borrow_mut().take() {
            let (controller, modem) =
                unsafe { &mut *self.driver.get() }.initialize(tx, rx, enable, reset);
            self.modem.mount(spawner, ModemActor::new(modem));
            self.wifi.mount(spawner, AdapterActor::new(controller))
        } else {
            panic!("Attempted to mount package twice!")
        }
    }
}

/// Convenience actor implementation of modem
pub struct ModemActor<'a, RX, ENABLE, RESET>
where
    RX: Read + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    modem: Esp8266Modem<'a, RX, ENABLE, RESET>,
}

impl<'a, RX, ENABLE, RESET> ModemActor<'a, RX, ENABLE, RESET>
where
    RX: Read + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    pub fn new(modem: Esp8266Modem<'a, RX, ENABLE, RESET>) -> Self {
        Self { modem }
    }
}

impl<'a, RX, ENABLE, RESET> Actor for ModemActor<'a, RX, ENABLE, RESET>
where
    RX: Read + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    type Message<'m> = ()
    where
        'a: 'm;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        'a: 'm,
        M: 'm + Inbox<Self>;

    fn on_mount<'m, M>(&'m mut self, _: Address<Self>, _: &'m mut M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            self.modem.run().await;
        }
    }
}

impl<'a, TX> super::Adapter for Esp8266Controller<'a, TX> where TX: Write {}
