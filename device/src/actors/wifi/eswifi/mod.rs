use embedded_hal::digital::v2::OutputPin;
//use crate::drivers::wifi::eswifi::*;
// use crate::kernel::{
//     actor::{Actor, ActorContext, ActorSpawner, Address, Inbox},
//     package::*,
// };
//use super::AdapterActor;

pub struct EsWifi<ENABLE, RESET>
where
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    enable_pin: ENABLE,
    reset_pin: RESET,
    // driver: UnsafeCell<Esp8266Driver>,
    // state: RefCell<Option<State<UART, ENABLE, RESET>>>,
    // wifi: ActorContext<'static, AdapterActor<Esp8266Controller<'static>>, 4>,
    // modem: ActorContext<'static, ModemActor<'static, UART, ENABLE, RESET>>,
}

impl<ENABLE, RESET> EsWifi<ENABLE, RESET>
where
    ENABLE: OutputPin + 'static,
    RESET: OutputPin + 'static,
{
    pub fn new(enable: ENABLE, reset: RESET) -> Self {
        info!("new EsWifi");
        Self {
            enable_pin: enable,
            reset_pin: reset,
            // driver: UnsafeCell::new(Esp8266Driver::new()),
            // state: RefCell::new(Some(State::New(uart, enable, reset))),
            // wifi: ActorContext::new(AdapterActor::new()),
            // modem: ActorContext::new(ModemActor::new()),
        }
    }
}

// impl<ENABLE, RESET> Package for EsWifi<ENABLE, RESET>
// where
//     ENABLE: OutputPin + 'static,
//     RESET: OutputPin + 'static,
// {
//     type Primary = AdapterActor<EsWifiController<'static, ENABLE, RESET>>;

//     fn mount<S: ActorSpawner>(
//         &'static self,
//         _: Self::Configuration,
//         spawner: S,
//     ) -> Address<Self::Primary> {

//         // if let Some(State::New(uart, enable, reset)) = self.state.borrow_mut().take() {
//         //     let (controller, modem) =
//         //         unsafe { &mut *self.driver.get() }.initialize(uart, enable, reset);
//         //     self.modem.mount(modem, spawner);
//         //     self.wifi.mount(controller, spawner)
//         // } else {
//         //     panic!("Attempted to mount package twice!")
//         // }
//     }
// }

// impl<'a, ENABLE, RESET> super::Adapter for EsWifiController<'a, ENABLE, RESET>
// where
//     ENABLE: OutputPin + 'static,
//     RESET: OutputPin + 'static,
//     {}