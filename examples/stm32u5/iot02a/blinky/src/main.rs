#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;

use core::future::Future;
use drogue_device::actors::button::{ButtonEvent, FromButtonEvent};
use drogue_device::actors::led::{Led, LedMessage};
use drogue_device::{actors::button::Button, Actor, ActorContext, Address, DeviceContext, Inbox};
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::peripherals::{PC13, PE13, PH6, PH7};
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    Peripherals,
};

type LedGreenPin = Output<'static, PH7>;
type LedBluePin = Output<'static, PH6>;
type LedRedPin = Output<'static, PE13>;

pub enum Color {
    Green,
    Blue,
    Red,
}

pub struct App {
    green: Option<Address<'static, Led<LedGreenPin>>>,
    yellow: Option<Address<'static, Led<LedBluePin>>>,
    red: Option<Address<'static, Led<LedRedPin>>>,
    color: Option<Color>,
}

impl App {
    fn draw(&self) {
        match self.color {
            None => {
                self.green.unwrap().notify(LedMessage::Off).ok();
                self.yellow.unwrap().notify(LedMessage::Off).ok();
                self.red.unwrap().notify(LedMessage::Off).ok();
            }
            Some(Color::Green) => {
                self.green.unwrap().notify(LedMessage::On).ok();
                self.yellow.unwrap().notify(LedMessage::Off).ok();
                self.red.unwrap().notify(LedMessage::Off).ok();
            }
            Some(Color::Blue) => {
                self.green.unwrap().notify(LedMessage::Off).ok();
                self.yellow.unwrap().notify(LedMessage::On).ok();
                self.red.unwrap().notify(LedMessage::Off).ok();
            }
            Some(Color::Red) => {
                self.green.unwrap().notify(LedMessage::Off).ok();
                self.yellow.unwrap().notify(LedMessage::Off).ok();
                self.red.unwrap().notify(LedMessage::On).ok();
            }
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self {
            color: Default::default(),
            green: Default::default(),
            yellow: Default::default(),
            red: Default::default(),
        }
    }
}

impl Actor for App {
    type Configuration = (
        Address<'static, Led<LedGreenPin>>,
        Address<'static, Led<LedBluePin>>,
        Address<'static, Led<LedRedPin>>,
    );

    type Message<'m> = Command;

    type OnMountFuture<'m, M>
    where
        M: 'm,
    = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        self.green.replace(config.0);
        self.yellow.replace(config.1);
        self.red.replace(config.2);
        async move {
            loop {
                match inbox.next().await {
                    Some(_) => match self.color {
                        None | Some(Color::Red) => {
                            self.color = Some(Color::Green);
                        }
                        Some(Color::Green) => {
                            self.color = Some(Color::Blue);
                        }
                        Some(Color::Blue) => {
                            self.color = Some(Color::Red);
                        }
                    },
                    _ => {}
                }

                self.draw();
            }
        }
    }
}

#[derive(Debug)]
pub enum Command {
    Next,
}

impl FromButtonEvent<Command> for App {
    fn from(event: ButtonEvent) -> Option<Command>
    where
        Self: Sized,
    {
        match event {
            ButtonEvent::Pressed => Some(Command::Next),
            ButtonEvent::Released => None,
        }
    }
}

pub struct MyDevice {
    app: ActorContext<'static, App>,
    led_green: ActorContext<'static, Led<LedGreenPin>>,
    led_blue: ActorContext<'static, Led<LedBluePin>>,
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
        led_green: ActorContext::new(Led::new(Output::new(p.PH7, Level::Low, Speed::Low))),
        led_blue: ActorContext::new(Led::new(Output::new(p.PH6, Level::Low, Speed::Low))),
        led_red: ActorContext::new(Led::new(Output::new(p.PE13, Level::Low, Speed::Low))),
        button: ActorContext::new(Button::new(button)),
    });

    DEVICE
        .mount(|device| async move {
            let green = device.led_green.mount((), spawner);
            let yellow = device.led_blue.mount((), spawner);
            let red = device.led_red.mount((), spawner);

            let app = device.app.mount((green, yellow, red), spawner);
            device.button.mount(app, spawner);
            app
        })
        .await;
    defmt::info!("Application initialized. Press 'A' button to cycle LEDs");
}
