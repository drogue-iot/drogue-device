use crate::actors::led::matrix::LedMatrixActor as MatrixActor;
use crate::bsp::Board;
use crate::drivers::{button::Button, led::matrix::LedMatrix as LedMatrixDriver, ActiveLow};
use crate::{
    domain::temperature::Celsius,
    domain::{temperature::Temperature, SensorAcquisition},
    traits::sensors::temperature::TemperatureSensor,
};
use core::future::Future;
#[cfg(feature = "bsp+microbit+temp")]
use embassy_nrf::interrupt;
use embassy_nrf::{
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin, Pull},
    peripherals::{
        P0_00, P0_01, P0_02, P0_03, P0_04, P0_08, P0_09, P0_10, P0_13, P0_14, P0_16, P0_23, P1_02,
        PPI_CH0, PPI_CH1, PWM0, RNG, TIMER0, TWISPI0, UARTE0,
    },
    pwm,
    temp::Temp,
};

pub type LedMatrix = LedMatrixDriver<Output<'static, AnyPin>, 5, 5>;
pub type LedMatrixActor = MatrixActor<Output<'static, AnyPin>, 5, 5>;

pub type PinButtonA = Input<'static, P0_14>;
pub type ButtonA = Button<Input<'static, P0_14>, ActiveLow>;

pub type PinButtonB = Input<'static, P0_23>;
pub type ButtonB = Button<Input<'static, P0_23>, ActiveLow>;

pub struct Microbit {
    pub led_matrix: LedMatrix,
    pub button_a: ButtonA,
    pub button_b: ButtonB,
    pub uarte0: UARTE0,
    pub timer0: TIMER0,
    pub p0_00: P0_00,
    pub p0_01: P0_01,
    pub p0_02: P0_02,
    pub p0_03: P0_03,
    pub p0_04: P0_04,
    pub p0_09: P0_09,
    pub p0_08: P0_08,
    pub p0_10: P0_10,
    pub p0_13: P0_13,
    pub p0_16: P0_16,
    pub p1_02: P1_02,
    pub twispi0: TWISPI0,
    pub pwm0: PWM0,
    pub ppi_ch0: PPI_CH0,
    pub ppi_ch1: PPI_CH1,
    #[cfg(feature = "bsp+microbit+temp")]
    pub temp: TemperatureMonitor,
    pub rng: RNG,
}

impl Board for Microbit {
    type Peripherals = embassy_nrf::Peripherals;
    type BoardConfig = ();
    fn new(p: embassy_nrf::Peripherals) -> Self {
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

        #[cfg(feature = "bsp+microbit+temp")]
        let temp_irq = interrupt::take!(TEMP);

        Self {
            led_matrix: LedMatrixDriver::new(rows, cols),
            button_a: Button::new(Input::new(p.P0_14, Pull::Up)),
            button_b: Button::new(Input::new(p.P0_23, Pull::Up)),
            uarte0: p.UARTE0,
            timer0: p.TIMER0,
            p0_00: p.P0_00,
            p0_01: p.P0_01,
            p0_02: p.P0_02,
            p0_03: p.P0_03,
            p0_04: p.P0_04,
            p0_08: p.P0_08,
            p0_09: p.P0_09,
            p0_10: p.P0_10,
            p0_13: p.P0_13,
            p0_16: p.P0_16,
            p1_02: p.P1_02,
            ppi_ch0: p.PPI_CH0,
            ppi_ch1: p.PPI_CH1,
            twispi0: p.TWISPI0,
            pwm0: p.PWM0,
            #[cfg(feature = "bsp+microbit+temp")]
            temp: TemperatureMonitor::new(Temp::new(p.TEMP, temp_irq)),
            rng: p.RNG,
        }
    }
}

fn output_pin(pin: AnyPin) -> Output<'static, AnyPin> {
    Output::new(pin, Level::Low, OutputDrive::Standard)
}

pub struct TemperatureMonitor {
    t: Temp<'static>,
}

impl TemperatureMonitor {
    pub fn new(t: Temp<'static>) -> Self {
        Self { t }
    }
}

impl TemperatureSensor<Celsius> for TemperatureMonitor {
    type Error = ();

    type CalibrateFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm;
    fn calibrate<'m>(&'m mut self) -> Self::CalibrateFuture<'m> {
        async move { Ok(()) }
    }

    type ReadFuture<'m> =
        impl Future<Output = Result<SensorAcquisition<Celsius>, Self::Error>> + 'm;

    fn temperature<'m>(&'m mut self) -> Self::ReadFuture<'m> {
        async move {
            let t = self.t.read().await;
            Ok(SensorAcquisition {
                temperature: Temperature::new(t.to_num::<f32>()),
                relative_humidity: 0.0,
            })
        }
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone, PartialEq)]
pub enum Pitch {
    C = 261,
    D = 293,
    E = 329,
    F = 349,
    G = 391,
    A = 440,
    AB = 466,
    B = 493,
    C2 = 523,
    Silent = 0,
}

#[derive(Clone, Copy)]
pub struct Note(pub Pitch, pub u32);

pub struct PwmSpeaker<'a, T: pwm::Instance> {
    pwm: pwm::SimplePwm<'a, T>,
}

impl<'a, T: pwm::Instance> PwmSpeaker<'a, T> {
    pub fn new(pwm: pwm::SimplePwm<'a, T>) -> Self {
        Self { pwm }
    }

    #[cfg(feature = "time")]
    pub async fn play_note(&mut self, note: Note) {
        use embassy::time::{Duration, Timer};
        if note.0 != Pitch::Silent {
            self.pwm.set_prescaler(pwm::Prescaler::Div4);
            self.pwm.set_period(note.0 as u32);
            self.pwm.enable();

            self.pwm.set_duty(0, self.pwm.max_duty() / 2);
            Timer::after(Duration::from_millis(note.1 as u64)).await;
            self.pwm.disable();
        } else {
            Timer::after(Duration::from_millis(note.1 as u64)).await;
        }
    }
}

/*
impl<'d, U: uarte::Instance, T: timer::Instance> crate::traits::serial::ReadUntilIdle
    for uarte::UarteRxWithIdle<'d, U, T>
{
    type ReadUntilIdleFuture<'m> = impl Future<Output = Result<usize, Self::Error>> where Self: 'm;
    fn read_until_idle<'m>(&'m mut self, buf: &'m mut [u8]) -> Self::ReadUntilIdleFuture<'m> {
        async move { self.read_until_idle(buf).await }
    }
}
*/
