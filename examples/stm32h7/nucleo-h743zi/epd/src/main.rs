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
use drogue_device::{
    actors::button::Button,
    Actor, ActorContext, Address, DeviceContext,
};
use embassy_stm32::dbgmcu::Dbgmcu;
use embassy_stm32::peripherals::*;
use embassy_stm32::{exti::ExtiInput, gpio::{Input, Level, Output, Pull, Speed}, Peripherals, Config};
use embassy_stm32::spi::{self, Spi};
use embassy_stm32::dma::NoDma;
use embassy_stm32::time::U32Ext;
use embassy::time::Delay;
use epd_waveshare::epd5in65f::{Epd5in65f, Display5in65f};
use epd_waveshare::prelude::*;
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use epd_waveshare::color::OctColor::{White, Black, Red, Blue};
use embedded_graphics::text::{TextStyleBuilder, Baseline, Text};
use embedded_graphics::geometry::Point;
use embedded_graphics::Drawable;

type EpdSpi = Spi<'static, SPI1, NoDma, NoDma>;
type Epd = Epd5in65f<EpdSpi, Output<'static, PD14>, Input<'static, PF3>, Output<'static, PG0>, Output<'static, PG1>, Delay>;

pub struct App {
    spi: EpdSpi,
    epd: Epd,
}

impl App {
    fn new(spi: EpdSpi, epd: Epd) -> Self {
        Self {
            spi,
            epd,
        }
    }

    fn draw(&mut self) {
        //fn draw_text(display: &mut Display4in2, text: &str, x: i32, y: i32) {
        let x = 20;
        let y = 20;
        let style_a = MonoTextStyleBuilder::new()
            .font(&embedded_graphics::mono_font::ascii::FONT_10X20)
            .text_color(Red)
            .background_color(White)
            .build();

        let style_b = MonoTextStyleBuilder::new()
            .font(&embedded_graphics::mono_font::ascii::FONT_10X20)
            .text_color(Blue)
            .background_color(White)
            .build();

        let text_style = TextStyleBuilder::new().baseline(Baseline::Top).build();

        let mut display = Display5in65f::default();
        let _ = Text::with_text_style("It's the weekend!", Point::new(x, y), style_a, text_style).draw(&mut display);
        let _ = Text::with_text_style("Time for cocktails!", Point::new(x, y+40), style_b, text_style).draw(&mut display);
        defmt::info!("READY TO UPDATE");
        defmt::unwrap!(self.epd.update_frame( &mut self.spi, display.buffer(), &mut Delay));
        defmt::info!("READY TO DISPLAY");
        defmt::unwrap!(self.epd.display_frame(&mut self.spi, &mut Delay));
        defmt::info!("DONE DISPLAY");
        //}
    }
}


impl Actor for App {
    type Configuration = ();

    #[rustfmt::skip]
    type Message<'m> = Command;

    fn on_mount(&mut self, _: Address<'static, Self>, config: Self::Configuration) {}

    #[rustfmt::skip]
    type OnStartFuture<'m> = impl Future<Output=()> + 'm;

    fn on_start(self: Pin<&'_ mut Self>) -> Self::OnStartFuture<'_> {
        async move {}
    }

    #[rustfmt::skip]
    type OnMessageFuture<'m> = impl Future<Output=()> + 'm;

    fn on_message<'m>(
        mut self: Pin<&'m mut Self>,
        _message: Self::Message<'m>,
    ) -> Self::OnMessageFuture<'m> {
        defmt::info!("message");
        async move {
            self.draw();
        }
    }
}

#[derive(Debug)]
pub enum Command {
    Draw,
}

impl FromButtonEvent<Command> for App {
    fn from(event: ButtonEvent) -> Option<Command>
        where
            Self: Sized,
    {
        match event {
            ButtonEvent::Pressed => Some(Command::Draw),
            ButtonEvent::Released => None,
        }
    }
}

pub struct MyDevice {
    app: ActorContext<'static, App>,
    button: ActorContext<'static, Button<'static, ExtiInput<'static, PC13>, App>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main(config="config()")]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    unsafe {
        Dbgmcu::enable_all();
    }

    let button = Input::new(p.PC13, Pull::Down);
    let button = ExtiInput::new(button, p.EXTI13);

    let mut spi = Spi::new(p.SPI1, p.PA5, p.PB5, p.PA6, NoDma, NoDma, 4.mhz(), spi::Config::default());
    let cs = Output::new(p.PD14, Level::High, Speed::Medium);
    let busy = Input::new(p.PF3, Pull::None);
    let dc = Output::new(p.PG0, Level::High, Speed::Medium);
    let rst = Output::new(p.PG1, Level::High, Speed::Medium);
    let mut delay = Delay;

    defmt::info!("initializing EPD");
    let mut epd = Epd5in65f::new(&mut spi, cs, busy, dc, rst, &mut delay);
    defmt::info!("completed initializing EPD");

    let mut epd = match epd {
        Ok(epd) => {
            defmt::info!("epd initialized");
            epd
        }
        Err(e) => {
            defmt::panic!("Error initializing EPD");
        }
    };

    DEVICE.configure(MyDevice {
        app: ActorContext::new(App::new(spi, epd)),
        button: ActorContext::new(Button::new(button)),
    });

    DEVICE
        .mount(|device| async move {
            let app = device.app.mount((), spawner);
            device.button.mount(app, spawner);
            app
        })
        .await;
    defmt::info!("Application initialized. Press 'A' button to draw");
}


#[allow(unused)]
pub fn config() -> Config {
    let mut config = Config::default();
    config.rcc.sys_ck = Some(400.mhz().into());
    config.rcc.pll1.q_ck = Some(100.mhz().into());
    config.rcc.enable_dma1 = true;
    config
}