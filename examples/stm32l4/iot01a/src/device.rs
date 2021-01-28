use stm32l4xx_hal::pac::{Interrupt, I2C2};
use stm32l4xx_hal::gpio::{PA5, Output, PushPull, PC13, Input, PullUp, OpenDrain, AF4, Alternate, PB10, PB11, PD15, PullDown, Floating, PB14};
use drogue_device::{
    prelude::*,
    synchronization::Mutex,
    driver::{
        sensor::hts221::{
            Hts221,
        },
        led::SimpleLED,
        button::Button,
    },
};
use stm32l4xx_hal::i2c::I2c;
use drogue_device::driver::sensor::hts221::Sensor;
use drogue_device::driver::timer::Timer;

type Ld1Actor = SimpleLED<MyDevice, PA5<Output<PushPull>>>;
type Ld2Actor = SimpleLED<MyDevice, PB14<Output<PushPull>>>;
type ButtonInterrupt = Button<MyDevice,PC13<Input<PullUp>>>;

type I2cScl = PB10<Alternate<AF4, Output<OpenDrain>>>;
type I2cSda = PB11<Alternate<AF4, Output<OpenDrain>>>;
type I2cPeriph = I2c<I2C2, (I2cScl, I2cSda)>;
type I2cActor = Mutex<MyDevice, I2cPeriph>;

use drogue_device::hal::timer::stm32l4xx::Timer as McuTimer;
use stm32l4xx_hal::pac::TIM15;
use drogue_device::driver::led::Blinker;
use drogue_device::driver::button::ButtonEvent;

type Blinker1Actor = Blinker<MyDevice, PA5<Output<PushPull>>, McuTimer<TIM15>>;
type Blinker2Actor = Blinker<MyDevice, PB14<Output<PushPull>>, McuTimer<TIM15>>;

type TimerActor = Timer<MyDevice, McuTimer<TIM15>>;

type Hts221Package = Hts221<MyDevice, PD15<Input<PullDown>>, I2cPeriph>;
type Hts221Sensor = Sensor<MyDevice, I2cPeriph>;

pub struct MyDevice {
    pub ld1: ActorContext<MyDevice, Ld1Actor>,
    pub ld2: ActorContext<MyDevice, Ld2Actor>,
    pub blinker1: ActorContext<MyDevice, Blinker1Actor>,
    pub blinker2: ActorContext<MyDevice, Blinker2Actor>,
    pub button: InterruptContext<MyDevice, ButtonInterrupt>,
    pub i2c: ActorContext<MyDevice, I2cActor>,
    pub hts221: Hts221Package,
    pub timer: InterruptContext<MyDevice, Timer<MyDevice, McuTimer<TIM15>>>,
}

impl Device for MyDevice {
    fn mount(&'static mut self, bus: &EventBus<Self>, supervisor: &mut Supervisor) {
        let ld1_addr = self.ld1.mount(bus, supervisor);
        let ld2_addr = self.ld2.mount(bus, supervisor);

        let blinker1_addr = self.blinker1.mount(bus, supervisor);
        let blinker2_addr = self.blinker2.mount(bus, supervisor);

        let i2c_addr = self.i2c.mount(bus, supervisor);
        let hts221_addr = self.hts221.mount(bus, supervisor);
        let timer_addr = self.timer.mount(bus, supervisor);

        blinker1_addr.bind(&timer_addr);
        blinker1_addr.bind(&ld1_addr);

        blinker2_addr.bind(&timer_addr);
        blinker2_addr.bind(&ld2_addr);


        hts221_addr.bind(&i2c_addr);

        let button_addr = self.button.mount(bus, supervisor);
    }
}

impl EventConsumer<ButtonEvent> for MyDevice {
    fn on_event(&'static mut self, message: ButtonEvent) where
        Self: Sized, {
        match message {
            ButtonEvent::Pressed => {
                log::info!("[event-bus] button pressed");
            }
            ButtonEvent::Released => {
                log::info!("[event-bus] button released");
            }
        }
    }
}


