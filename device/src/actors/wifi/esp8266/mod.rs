use super::AdapterActor;
use crate::drivers::wifi::esp8266::*;
use crate::kernel::{
    actor::{Actor, ActorContext, ActorSpawner, Address, Inbox},
    package::*,
};
use core::{
    cell::{RefCell, UnsafeCell},
    future::Future,
    pin::Pin,
};
use embassy::io::{AsyncBufReadExt, AsyncWriteExt};
use embedded_hal::digital::v2::OutputPin;

pub enum State<UART, ENABLE, RESET>
where
    UART: AsyncBufReadExt + AsyncWriteExt + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    New(UART, ENABLE, RESET),
    Initialized,
}

pub struct Esp8266Wifi<UART, ENABLE, RESET>
where
    UART: AsyncBufReadExt + AsyncWriteExt + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    driver: UnsafeCell<Esp8266Driver>,
    state: RefCell<Option<State<UART, ENABLE, RESET>>>,
    wifi: ActorContext<'static, AdapterActor<Esp8266Controller<'static>>, 4>,
    modem: ActorContext<'static, ModemActor<'static, UART, ENABLE, RESET>>,
}

impl<UART, ENABLE, RESET> Esp8266Wifi<UART, ENABLE, RESET>
where
    UART: AsyncBufReadExt + AsyncWriteExt + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    pub fn new(uart: UART, enable: ENABLE, reset: RESET) -> Self {
        Self {
            driver: UnsafeCell::new(Esp8266Driver::new()),
            state: RefCell::new(Some(State::New(uart, enable, reset))),
            wifi: ActorContext::new(AdapterActor::new()),
            modem: ActorContext::new(ModemActor::new()),
        }
    }
}

impl<UART, ENABLE, RESET> Package for Esp8266Wifi<UART, ENABLE, RESET>
where
    UART: AsyncBufReadExt + AsyncWriteExt + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    type Primary = AdapterActor<Esp8266Controller<'static>>;

    fn mount<S: ActorSpawner>(
        &'static self,
        _: Self::Configuration,
        spawner: S,
    ) -> Address<Self::Primary> {
        if let Some(State::New(uart, enable, reset)) = self.state.borrow_mut().take() {
            let (controller, modem) =
                unsafe { &mut *self.driver.get() }.initialize(uart, enable, reset);
            self.modem.mount(modem, spawner);
            self.wifi.mount(controller, spawner)
        } else {
            panic!("Attempted to mount package twice!")
        }
    }
}

/// Convenience actor implementation of modem
pub struct ModemActor<'a, UART, ENABLE, RESET>
where
    UART: AsyncBufReadExt + AsyncWriteExt + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    modem: Option<Esp8266Modem<'a, UART, ENABLE, RESET>>,
}

impl<'a, UART, ENABLE, RESET> ModemActor<'a, UART, ENABLE, RESET>
where
    UART: AsyncBufReadExt + AsyncWriteExt + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    pub fn new() -> Self {
        Self { modem: None }
    }
}

impl<'a, UART, ENABLE, RESET> Unpin for ModemActor<'a, UART, ENABLE, RESET>
where
    UART: AsyncBufReadExt + AsyncWriteExt + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
}

impl<'a, UART, ENABLE, RESET> Actor for ModemActor<'a, UART, ENABLE, RESET>
where
    UART: AsyncBufReadExt + AsyncWriteExt + 'static,
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    type Configuration = Esp8266Modem<'a, UART, ENABLE, RESET>;
    #[rustfmt::skip]
    type Message<'m> where 'a: 'm = ();

    fn on_mount(&mut self, _: Address<'static, Self>, config: Self::Configuration) {
        self.modem.replace(config);
    }

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where 'a: 'm, M: 'm = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(&'m mut self, _: Self::Configuration, _: Address<'static, Self>, _: &'m mut M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            self.modem.as_mut().unwrap().run().await;
        }
    }
}

impl<'a> super::Adapter for Esp8266Controller<'a> {}
