#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use defmt_rtt as _;
use drogue_device::{
    actors::led::matrix::LedMatrixActor,
    drivers::led::matrix::LedMatrix,
    traits::led::{LedMatrix as LedMatrixTrait, TextDisplay},
    ActorContext, DeviceContext,
};

use embassy::time::{Duration, Instant, Timer};
use embassy_nrf::{
    gpio::{AnyPin, Level, NoPin, Output, OutputDrive, Pin},
    interrupt,
    peripherals::PWM0,
    pwm::*,
    twim, Peripherals,
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

use micromath::F32Ext;

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
        for i in 0..BEAT.len() {
            speaker.play_sample(&BEAT[i]).await;
        }
    }
}

static BEAT: &[Sample<'static>] = &[
    Sample::new(&[
        Note(440, 150),
        Note(0, 100),
        Note(440, 150),
        Note(0, 250),
        Note(440, 300),
        Note(0, 50),
        Note(440, 150),
        Note(0, 800),
    ]),
    Sample::new(&[
        Note(659, 150),
        Note(0, 100),
        Note(659, 150),
        Note(0, 250),
        Note(659, 300),
        Note(0, 50),
        Note(659, 150),
        Note(0, 800),
    ]),
    Sample::new(&[
        Note(440, 150),
        Note(0, 100),
        Note(440, 150),
        Note(0, 250),
        Note(440, 300),
        Note(0, 50),
        Note(440, 150),
        Note(0, 800),
    ]),
    Sample::new(&[
        Note(659, 150),
        Note(0, 100),
        Note(659, 150),
        Note(0, 250),
        Note(659, 300),
        Note(0, 50),
        Note(659, 150),
        Note(0, 800),
    ]),
    Sample::new(&[
        Note(587, 150),
        Note(0, 100),
        Note(587, 150),
        Note(0, 250),
        Note(587, 300),
        Note(0, 50),
        Note(587, 150),
        Note(0, 800),
    ]),
    Sample::new(&[
        Note(587, 150),
        Note(0, 100),
        Note(587, 150),
        Note(0, 250),
        Note(587, 300),
        Note(0, 50),
        Note(587, 150),
        Note(0, 300),
        Note(587, 100),
        Note(659, 400),
    ]),
    Sample::new(&[
        Note(440, 150),
        Note(0, 100),
        Note(440, 150),
        Note(0, 250),
        Note(440, 300),
        Note(0, 50),
        Note(440, 150),
        Note(0, 800),
    ]),
    Sample::new(&[
        Note(440, 150),
        Note(0, 100),
        Note(440, 150),
        Note(0, 250),
        Note(440, 300),
        Note(0, 50),
        Note(440, 150),
        Note(0, 800),
    ]),
];
