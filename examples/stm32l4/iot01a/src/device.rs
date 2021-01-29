use drogue_device::domain::time::duration::Milliseconds;
use drogue_device::driver::sensor::hts221::SensorAcquisition;
use drogue_device::{
    driver::{
        button::{Button, ButtonEvent},
        led::{Blinker, SimpleLED},
        sensor::hts221::Hts221,
        timer::Timer,
    },
    hal::timer::stm32l4xx::Timer as McuTimer,
    prelude::*,
    synchronization::Mutex,
};
use stm32l4xx_hal::{
    gpio::{
        Alternate, Input, OpenDrain, Output, PullDown, PullUp, PushPull, AF4, PA5, PB10, PB11,
        PB14, PC13, PD15,
    },
    i2c::I2c,
    pac::I2C2,
    pac::TIM15,
};

type Ld1Actor = SimpleLED<PA5<Output<PushPull>>>;
type Ld2Actor = SimpleLED<PB14<Output<PushPull>>>;
type ButtonInterrupt = Button<MyDevice, PC13<Input<PullUp>>>;

type I2cScl = PB10<Alternate<AF4, Output<OpenDrain>>>;
type I2cSda = PB11<Alternate<AF4, Output<OpenDrain>>>;
type I2cPeriph = I2c<I2C2, (I2cScl, I2cSda)>;
type I2cActor = Mutex<I2cPeriph>;

type Blinker1Actor = Blinker<PA5<Output<PushPull>>, McuTimer<TIM15>>;
type Blinker2Actor = Blinker<PB14<Output<PushPull>>, McuTimer<TIM15>>;

type TimerActor = Timer<McuTimer<TIM15>>;

type Hts221Package = Hts221<MyDevice, PD15<Input<PullDown>>, I2cPeriph>;
//type Hts221Sensor = Sensor<MyDevice, I2cPeriph>;

pub struct MyDevice {
    pub ld1: ActorContext<Ld1Actor>,
    pub ld2: ActorContext<Ld2Actor>,
    pub blinker1: ActorContext<Blinker1Actor>,
    pub blinker2: ActorContext<Blinker2Actor>,
    pub button: InterruptContext<ButtonInterrupt>,
    pub i2c: ActorContext<I2cActor>,
    pub hts221: Hts221Package,
    pub timer: InterruptContext<TimerActor>,
}

impl Device for MyDevice {
    fn mount(
        &'static mut self,
        bus_address: &Address<EventBus<Self>>,
        supervisor: &mut Supervisor,
    ) {
        let ld1_addr = self.ld1.mount(supervisor);
        let ld2_addr = self.ld2.mount(supervisor);

        let blinker1_addr = self.blinker1.mount(supervisor);
        let blinker2_addr = self.blinker2.mount(supervisor);

        let i2c_addr = self.i2c.mount(supervisor);
        let hts221_addr = self.hts221.mount(bus_address, supervisor);
        let timer_addr = self.timer.mount(supervisor);

        blinker1_addr.bind(&timer_addr);
        blinker1_addr.bind(&ld1_addr);

        blinker2_addr.bind(&timer_addr);
        blinker2_addr.bind(&ld2_addr);

        hts221_addr.bind(&i2c_addr);

        let button_addr = self.button.mount(supervisor);
        button_addr.bind(bus_address);
    }
}

impl EventHandler<ButtonEvent> for MyDevice {
    fn on_event(&'static mut self, message: ButtonEvent)
    where
        Self: Sized,
    {
        match message {
            ButtonEvent::Pressed => {
                log::info!("[{}] button pressed", ActorInfo::name());
                self.blinker1.address().adjust_delay(Milliseconds(100u32));
            }
            ButtonEvent::Released => {
                log::info!("[{}] button released", ActorInfo::name());
                self.blinker1.address().adjust_delay(Milliseconds(500u32));
            }
        }
    }
}

impl EventHandler<SensorAcquisition> for MyDevice {
    fn on_event(&'static mut self, message: SensorAcquisition)
    where
        Self: Sized,
    {
        log::info!("[event-bus] {:?}", message);
    }
}
