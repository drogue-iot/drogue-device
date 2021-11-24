#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use drogue_device::{
    actors::led::matrix::LedMatrixActor, drivers::led::matrix::LedMatrix, ActorContext,
    DeviceContext,
};

use embassy_nrf::{
    gpio::{AnyPin, Level, NoPin, Output, OutputDrive, Pin},
    pwm::*,
    Peripherals,
};

use panic_probe as _;

mod speaker;
use speaker::*;

pub type AppMatrix = LedMatrixActor<Output<'static, AnyPin>, 5, 5>;

pub struct MyDevice {
    matrix: ActorContext<'static, AppMatrix>,
}
static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

fn output_pin(pin: AnyPin) -> Output<'static, AnyPin> {
    Output::new(pin, Level::Low, OutputDrive::Standard)
}

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    // LED Matrix
    let rows = [
        output_pin(p.P0_21.degrade()),
        output_pin(p.P0_22.degrade()),
        output_pin(p.P0_15.degrade()),
        output_pin(p.P0_24.degrade()),
        output_pin(p.P0_19.degrade()),
    ];

    let cols = [
        output_pin(p.P0_28.degrade()),
        output_pin(p.P0_11.degrade()),
        output_pin(p.P0_31.degrade()),
        output_pin(p.P1_05.degrade()),
        output_pin(p.P0_30.degrade()),
    ];
    let led = LedMatrix::new(rows, cols);

    DEVICE.configure(MyDevice {
        matrix: ActorContext::new(LedMatrixActor::new(led, None)),
    });

    let matrix = DEVICE
        .mount(|device| async move {
            let matrix = device.matrix.mount((), spawner);
            matrix
        })
        .await;

    let pwm = SimplePwm::new(p.PWM0, p.P0_00, NoPin, NoPin, NoPin);
    let mut speaker = PwmSpeaker::new(pwm, matrix);

    loop {
        for i in 0..RIFF.len() {
            speaker.play_sample(&RIFF[i]).await;
        }
    }
}

static RIFF: &[Sample<'static>] = &[Sample::new(&[
    Note(Pitch::E as u32, 150),
    Note(0, 150),
    Note(Pitch::G as u32, 150),
    Note(0, 150),
    Note(Pitch::A as u32, 300),
    Note(0, 150),
    Note(Pitch::E as u32, 150),
    Note(0, 150),
    Note(Pitch::G as u32, 150),
    Note(0, 150),
    Note(Pitch::AB as u32, 150),
    Note(0, 25),
    Note(Pitch::A as u32, 300),
    Note(0, 300),
    Note(Pitch::E as u32, 150),
    Note(0, 150),
    Note(Pitch::G as u32, 150),
    Note(0, 150),
    Note(Pitch::A as u32, 300),
    Note(0, 150),
    Note(Pitch::G as u32, 150),
    Note(0, 150),
    Note(Pitch::E as u32, 300),
    Note(0, 750),
])];
