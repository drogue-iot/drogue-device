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
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::peripherals::{PB0, PB14, PC13, PE1};
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    Peripherals,
};

type LedGreenPin = Output<'static, PB0>;
type LedYellowPin = Output<'static, PE1>;
type LedRedPin = Output<'static, PB14>;

pub enum Color {
    Green,
    Yellow,
    Red,
}

pub struct App {
    green: Option<Address<'static, Led<LedGreenPin>>>,
    yellow: Option<Address<'static, Led<LedYellowPin>>>,
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
            Some(Color::Yellow) => {
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
                    Some((_, r)) => r.respond(match self.color {
                        None | Some(Color::Red) => {
                            self.color = Some(Color::Green);
                        }
                        Some(Color::Green) => {
                            self.color = Some(Color::Yellow);
                        }
                        Some(Color::Yellow) => {
                            self.color = Some(Color::Red);
                        }
                    }),
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

    DEVICE
        .mount(|device| async move {
            let green = device.led_green.mount((), spawner);
            let yellow = device.led_yellow.mount((), spawner);
            let red = device.led_red.mount((), spawner);

            let app = device.app.mount((green, yellow, red), spawner);
            device.button.mount(app, spawner);
            app
        })
        .await;
    defmt::info!("Application initialized. Press 'A' button to cycle LEDs");
}
