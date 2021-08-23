#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;

use core::future::Future;
use core::pin::Pin;
use drogue_device::actors::button::{ButtonEvent, FromButtonEvent};
use drogue_device::actors::led::{Led, LedMessage};
use drogue_device::{actors::button::Button, Actor, ActorContext, Address, DeviceContext, Inbox};
use embassy::time::{with_timeout, Duration};
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::peripherals::{PB0, PB14, PC13, PE1, RNG};
use embassy_stm32::rng::Random;
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    Peripherals,
};
use rand_core::RngCore;

type LedGreenPin = Output<'static, PB0>;
type LedYellowPin = Output<'static, PE1>;
type LedRedPin = Output<'static, PB14>;

pub enum Color {
    Green,
    Yellow,
    Red,
}

#[derive(Debug)]
pub enum Command {
    StartDancing,
    StopDancing,
}

pub struct App {
    address: Option<Address<'static, App>>,
    rng: Option<Random<RNG>>,
    dancing: bool,
    green: Option<Address<'static, Led<LedGreenPin>>>,
    yellow: Option<Address<'static, Led<LedYellowPin>>>,
    red: Option<Address<'static, Led<LedRedPin>>>,
}

impl App {
    fn all_on(&self) {
        self.green.unwrap().notify(LedMessage::On);
        self.yellow.unwrap().notify(LedMessage::On);
        self.red.unwrap().notify(LedMessage::On);
    }

    fn all_off(&self) {
        self.green.unwrap().notify(LedMessage::Off);
        self.yellow.unwrap().notify(LedMessage::Off);
        self.red.unwrap().notify(LedMessage::Off);
    }

    fn randomize(&mut self) {
        let val = self.rng.as_mut().unwrap().next_u32();

        let green = (val & 0b001) != 0;
        let yellow = (val & 0b010) != 0;
        let red = (val & 0b100) != 0;

        if green {
            self.green.unwrap().notify(LedMessage::On);
        } else {
            self.green.unwrap().notify(LedMessage::Off);
        }
        if yellow {
            self.yellow.unwrap().notify(LedMessage::On);
        } else {
            self.yellow.unwrap().notify(LedMessage::Off);
        }
        if red {
            self.red.unwrap().notify(LedMessage::On);
        } else {
            self.red.unwrap().notify(LedMessage::Off);
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            dancing: false,
            address: Default::default(),
            rng: Default::default(),
            green: Default::default(),
            yellow: Default::default(),
            red: Default::default(),
        }
    }
}

impl Actor for App {
    type Configuration = (
        Random<RNG>,
        Address<'static, Led<LedGreenPin>>,
        Address<'static, Led<LedYellowPin>>,
        Address<'static, Led<LedRedPin>>,
    );

    #[rustfmt::skip]
    type Message<'m> = Command;

    #[rustfmt::skip]
    type OnMountFuture<'m, M> where M: 'm = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        address: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        self.address.replace(address);
        self.rng.replace(config.0);
        self.green.replace(config.1);
        self.yellow.replace(config.2);
        self.red.replace(config.3);
        async move {
            loop {
                match with_timeout(Duration::from_millis(50), inbox.next()).await {
                    Ok(Some((message, r))) => r.respond(match message {
                        Command::StartDancing => {
                            self.all_on();
                            self.dancing = true;
                        }
                        Command::StopDancing => {
                            self.dancing = false;
                            self.all_off();
                        }
                    }),
                    _ => {}
                }
                if self.dancing {
                    self.randomize();
                }
            }
        }
    }
}

impl FromButtonEvent<Command> for App {
    fn from(event: ButtonEvent) -> Option<Command>
    where
        Self: Sized,
    {
        match event {
            ButtonEvent::Released => Some(Command::StartDancing),
            ButtonEvent::Pressed => Some(Command::StopDancing),
        }
    }
}

pub struct MyDevice {
    app: ActorContext<'static, App>,
    led_green: ActorContext<'static, Led<LedGreenPin>>,
    led_yellow: ActorContext<'static, Led<LedYellowPin>>,
    led_red: ActorContext<'static, Led<LedRedPin>>,
    button: ActorContext<'static, Button<'static, ExtiInput<'static, PC13>, App>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
    }

    let button = Input::new(p.PC13, Pull::Down);
    let button = ExtiInput::new(button, p.EXTI13);

    DEVICE.configure(MyDevice {
        app: ActorContext::new(App::default()),
        led_green: ActorContext::new(Led::new(Output::new(p.PB0, Level::Low, Speed::Low))),
        led_yellow: ActorContext::new(Led::new(Output::new(p.PE1, Level::Low, Speed::Low))),
        led_red: ActorContext::new(Led::new(Output::new(p.PB14, Level::Low, Speed::Low))),
        button: ActorContext::new(Button::new(button)),
    });

    let rng = Random::new(p.RNG);

    DEVICE
        .mount(|device| async move {
            let green = device.led_green.mount((), spawner);
            let yellow = device.led_yellow.mount((), spawner);
            let red = device.led_red.mount((), spawner);

            let app = device.app.mount((rng, green, yellow, red), spawner);
            device.button.mount(app, spawner);
            app
        })
        .await;
    defmt::info!("Application initialized. Press 'A' button to cycle LEDs");
}
