#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;

use core::future::Future;
use drogue_device::actors::button::{Button, ButtonEvent, ButtonEventDispatcher, FromButtonEvent};
use drogue_device::actors::led::{ActiveHigh, ActiveLow, Led, LedMessage};
use drogue_device::{Actor, ActorContext, Address, DeviceContext, Inbox};
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::peripherals::{PC13, PE13, PH6, PH7};
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    Peripherals,
};

type LedBluePin = Output<'static, PE13>;
type LedGreenPin = Output<'static, PH7>;
type LedRedPin = Output<'static, PH6>;

pub enum Color {
    Green,
    Blue,
    Red,
}

pub struct App {
    green: Option<Address<'static, Led<LedGreenPin, ActiveLow>>>,
    blue: Option<Address<'static, Led<LedBluePin, ActiveHigh>>>,
    red: Option<Address<'static, Led<LedRedPin, ActiveLow>>>,
    color: Option<Color>,
}

impl App {
    fn draw(&self) {
        match self.color {
            None => {
                defmt::info!("none");
                self.green.unwrap().notify(LedMessage::Off).ok();
                self.blue.unwrap().notify(LedMessage::Off).ok();
                self.red.unwrap().notify(LedMessage::Off).ok();
            }
            Some(Color::Green) => {
                defmt::info!("green");
                self.green.unwrap().notify(LedMessage::On).ok();
                self.blue.unwrap().notify(LedMessage::Off).ok();
                self.red.unwrap().notify(LedMessage::Off).ok();
            }
            Some(Color::Blue) => {
                defmt::info!("blue");
                self.green.unwrap().notify(LedMessage::Off).ok();
                self.blue.unwrap().notify(LedMessage::On).ok();
                self.red.unwrap().notify(LedMessage::Off).ok();
            }
            Some(Color::Red) => {
                defmt::info!("red");
                self.green.unwrap().notify(LedMessage::Off).ok();
                self.blue.unwrap().notify(LedMessage::Off).ok();
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
            blue: Default::default(),
            red: Default::default(),
        }
    }
}

impl Actor for App {
    type Configuration = (
        Address<'static, Led<LedGreenPin, ActiveLow>>,
        Address<'static, Led<LedBluePin, ActiveHigh>>,
        Address<'static, Led<LedRedPin, ActiveLow>>,
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
        self.blue.replace(config.1);
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
    led_red: ActorContext<'static, Led<LedRedPin, ActiveLow>>,
    led_green: ActorContext<'static, Led<LedGreenPin, ActiveLow>>,
    led_blue: ActorContext<'static, Led<LedBluePin, ActiveHigh>>,
    button: ActorContext<'static, Button<ExtiInput<'static, PC13>, ButtonEventDispatcher<App>>>,
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
        led_red: ActorContext::new(Led::new(Output::new(p.PH6, Level::High, Speed::Low))),
        led_green: ActorContext::new(Led::new(Output::new(p.PH7, Level::High, Speed::Low))),
        led_blue: ActorContext::new(Led::new(Output::new(p.PE13, Level::Low, Speed::Low))),
        button: ActorContext::new(Button::new(button)),
    });

    DEVICE
        .mount(|device| async move {
            let green = device.led_green.mount((), spawner);
            let blue = device.led_blue.mount((), spawner);
            let red = device.led_red.mount((), spawner);

            let app = device.app.mount((green, blue, red), spawner);
            device.button.mount(app.into(), spawner);
            app
        })
        .await;
    defmt::info!("Application initialized. Press the blue button to cycle LEDs");
}
