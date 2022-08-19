#![no_std]
#![no_main]
#![feature(trait_alias)]
#![feature(type_alias_impl_trait)]
#![allow(incomplete_features)]
#![feature(generic_associated_types)]

use defmt_rtt as _;
use panic_probe as _;

use core::fmt::Write;
use drogue_device::actors::button::{Button, ButtonEvent};
use ector::{Actor, ActorContext, Address, Inbox};
use embassy_time::Delay;
use embassy_util::Forever;
use embassy_stm32::dma::NoDma;
use embassy_stm32::peripherals::*;
use embassy_stm32::rng::Rng as Random;
use embassy_stm32::spi::{self, Spi};
use embassy_stm32::time::U32Ext;
use embassy_stm32::{
    exti::ExtiInput,
    gpio::{Input, Level, Output, Pull, Speed},
    Config, Peripherals,
};
use embedded_graphics::geometry::Point;
use embedded_graphics::image::{Image, ImageRaw};
use embedded_graphics::mono_font::MonoTextStyleBuilder;
use embedded_graphics::pixelcolor::{Rgb888, RgbColor};
use embedded_graphics::text::{Baseline, Text, TextStyleBuilder};
use embedded_graphics::Drawable;
use epd_waveshare::epd5in65f::{Display5in65f, Epd5in65f};
use epd_waveshare::prelude::*;
use heapless::{String, Vec};
use tinybmp::Bmp;

type EpdSpi = Spi<'static, SPI1, NoDma, NoDma>;
type Epd = Epd5in65f<
    EpdSpi,
    Output<'static, PD14>,
    Input<'static, PF3>,
    Output<'static, PG0>,
    Output<'static, PG1>,
    Delay,
>;

pub struct App {
    spi: EpdSpi,
    epd: Epd,
    presses: u16,
    random: Random<'static, RNG>,
}

impl App {
    fn new(spi: EpdSpi, epd: Epd, random: Random<'static, RNG>) -> Self {
        Self {
            spi,
            epd,
            presses: 0,
            random,
        }
    }

    async fn random_color(&mut self) -> (OctColor, OctColor) {
        let mut v = [0; 1];
        self.random.async_fill_bytes(&mut v).await.ok();

        let fg = v[0] & 0b00000111;
        let bg = (v[0] & 0b01110000) >> 4;

        (
            OctColor::from_nibble(fg).unwrap_or(OctColor::Black),
            OctColor::from_nibble(bg).unwrap_or(OctColor::White),
        )
    }

    async fn draw(&mut self) {
        let x = 20;
        let y = 20;
        let colors = self.random_color().await;
        let style_a = MonoTextStyleBuilder::new()
            .font(&embedded_graphics::mono_font::ascii::FONT_10X20)
            .text_color(colors.0)
            .background_color(colors.1)
            .build();

        let style_b = MonoTextStyleBuilder::new()
            .font(&embedded_graphics::mono_font::ascii::FONT_10X20)
            .text_color(OctColor::Blue)
            .background_color(OctColor::White)
            .build();

        let text_style = TextStyleBuilder::new().baseline(Baseline::Top).build();

        let mut display = Display5in65f::default();

        let mut text: String<128> = String::new();
        write!(text, "Drawing #{}", self.presses).ok();

        let _ = defmt::unwrap!(Text::with_text_style(
            text.as_str(),
            Point::new(x, y),
            style_a,
            text_style
        )
        .draw(&mut display));
        let _ = defmt::unwrap!(Text::with_text_style(
            "Powered by Drogue Device",
            Point::new(x, y + 40),
            style_b,
            text_style
        )
        .draw(&mut display));

        let bmp_data = include_bytes!("rodney.bmp");
        let bmp = Bmp::<Rgb888>::from_slice(bmp_data).unwrap();
        let width = bmp.as_raw().header().image_size.width;

        // 32kb frame buffer, more than enough.
        let mut image_data: Vec<u8, 32767> = Vec::new();

        let mut high = true;
        let mut current_pixel = 0u8;
        for pixel in bmp.pixels().into_iter() {
            // two pixels stuffed into each byte of the frame buffer.
            if high {
                current_pixel = quantize_color(pixel.1).get_nibble() << 4;
            } else {
                current_pixel = current_pixel | quantize_color(pixel.1).get_nibble();
                defmt::unwrap!(image_data.push(current_pixel));
                current_pixel = 0;
            }

            high = !high;
        }

        let image = ImageRaw::<OctColor>::new(&*image_data, width);

        let _ = Image::new(&image, Point::new(100, 100)).draw(&mut display);

        defmt::unwrap!(self
            .epd
            .update_frame(&mut self.spi, display.buffer(), &mut Delay));
        defmt::unwrap!(self.epd.display_frame(&mut self.spi, &mut Delay));
    }
}

fn quantize_color(color: Rgb888) -> OctColor {
    let r = color.r();
    let g = color.g();
    let b = color.b();

    if r == 0xFF && g == 0xFF && b == 0xFF {
        OctColor::White
    } else if r == 0 && g == 0 && b == 0 {
        OctColor::Black
    } else if r > g && g > b {
        OctColor::Yellow
    } else if r > g && r > b {
        OctColor::Red
    } else if g > r && g > b {
        OctColor::Green
    } else if b > r && b > g {
        OctColor::Blue
    } else {
        OctColor::Black
    }
}

#[ector::actor]
impl Actor for App {
    type Message<'m> = Command;

    async fn on_mount<M>(&mut self, _: Address<Self::Message<'m>>, mut inbox: M)
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        loop {
            let _msg = inbox.next().await;
            self.presses += 1;
            self.draw().await;
        }
    }
}

#[derive(Debug)]
pub enum Command {
    Draw,
}

impl core::convert::TryFrom<ButtonEvent> for Command {
    type Error = ();
    fn try_from(event: ButtonEvent) -> Result<Self, Self::Error> {
        match event {
            ButtonEvent::Pressed => Ok(Command::Draw),
            ButtonEvent::Released => Err(()),
        }
    }
}

pub struct MyDevice {
    app: ActorContext<App>,
    button: ActorContext<Button<ExtiInput<'static, PC13>, Command>>,
}

static DEVICE: Forever<MyDevice> = Forever::new();

#[embassy_executor::main(config = "config()")]
async fn main(spawner: embassy_executor::Spawner, p: Peripherals) {
    let button = Input::new(p.PC13, Pull::Down);
    let button = ExtiInput::new(button, p.EXTI13);

    let mut spi = Spi::new(
        p.SPI1,
        p.PA5,
        p.PB5,
        p.PA6,
        NoDma,
        NoDma,
        4.mhz(),
        spi::Config::default(),
    );
    let cs = Output::new(p.PD14, Level::High, Speed::Medium);
    let busy = Input::new(p.PF3, Pull::None);
    let dc = Output::new(p.PG0, Level::High, Speed::Medium);
    let rst = Output::new(p.PG1, Level::High, Speed::Medium);
    let mut delay = Delay;

    let epd = Epd5in65f::new(&mut spi, cs, busy, dc, rst, &mut delay);

    let epd = match epd {
        Ok(epd) => epd,
        Err(_) => {
            defmt::panic!("Error initializing EPD");
        }
    };

    let rng = Random::new(p.RNG);

    let device = DEVICE.put(MyDevice {
        app: ActorContext::new(),
        button: ActorContext::new(),
    });
    let app = device.app.mount(spawner, App::new(spi, epd, rng));
    device
        .button
        .mount(spawner, Button::new(button, app.into()));
    defmt::info!("Application initialized. Press the blue button to draw");
}

#[allow(unused)]
pub fn config() -> Config {
    let mut config = Config::default();
    config.rcc.sys_ck = Some(400.mhz().into());
    config.rcc.pll1.q_ck = Some(100.mhz().into());
    config.enable_debug_during_sleep = true;
    config
}
