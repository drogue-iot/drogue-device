use drogue_device::device::DeviceConfiguration;
use drogue_device::domain::time::duration::Milliseconds;
use drogue_device::driver::sensor::hts221::SensorAcquisition;
use drogue_device::driver::wifi::eswifi::EsWifi;
use drogue_device::{
    domain::temperature::Celsius,
    driver::{
        i2c::I2c,
        memory::{Memory, Query},
        spi::Spi,
    },
    hal::gpio::ActiveHigh,
};
use drogue_device::{
    driver::{
        button::{Button, ButtonEvent},
        led::{Blinker, SimpleLED},
        sensor::hts221::Hts221,
        timer::Timer,
    },
    hal::timer::stm32l4xx::Timer as HardwareTimer,
    prelude::*,
};
use stm32l4xx_hal::gpio::{PB13, PE0, PE1, PE13, PE8};
use stm32l4xx_hal::{
    gpio::{
        Alternate, Floating, Input, OpenDrain, Output, PullDown, PullUp, PushPull, AF4, AF6, PA5,
        PB10, PB11, PB14, PC10, PC11, PC12, PC13, PD15,
    },
    i2c::I2c as HalI2c,
    pac::I2C2,
    pac::SPI3,
    pac::TIM15,
    spi::Spi as HalSpi,
};

type Ld1Pin = PA5<Output<PushPull>>;
type Ld2Pin = PB14<Output<PushPull>>;

type Ld1Actor = SimpleLED<Ld1Pin, ActiveHigh>;
type Ld2Actor = SimpleLED<Ld2Pin, ActiveHigh>;
type ButtonInterrupt = Button<MyDevice, PC13<Input<PullUp>>>;

type I2cScl = PB10<Alternate<AF4, Output<OpenDrain>>>;
type I2cSda = PB11<Alternate<AF4, Output<OpenDrain>>>;
type I2cPeriph = HalI2c<I2C2, (I2cScl, I2cSda)>;
type I2cPackage = I2c<I2cPeriph>;

type TimerPackage = Timer<HardwareTimer<TIM15>>;

type Blinker1Actor = Blinker<Ld1Actor, <TimerPackage as Package>::Primary>;
type Blinker2Actor = Blinker<Ld2Actor, <TimerPackage as Package>::Primary>;

type Hts221Package = Hts221<MyDevice, PD15<Input<PullDown>>, I2cPeriph>;

type SpiClk = PC10<Alternate<AF6, Input<Floating>>>;
type SpiMiso = PC11<Alternate<AF6, Input<Floating>>>;
type SpiMosi = PC12<Alternate<AF6, Input<Floating>>>;

type HardwareSpi = HalSpi<SPI3, (SpiClk, SpiMiso, SpiMosi)>;
type SpiPackage = Spi<HardwareSpi, u8>;

type WifiCs = PE0<Output<PushPull>>;
//type WifiCs = PE10<Output<PullUp>>;
type WifiReady = PE1<Input<PullDown>>;
type WifiReset = PE8<Output<PushPull>>;
type WifiWakeup = PB13<Output<PushPull>>;

pub struct MyDevice {
    pub spi: SpiPackage,
    pub wifi: EsWifi<
        HardwareSpi,
        <TimerPackage as Package>::Primary,
        WifiCs,
        WifiReady,
        WifiReset,
        WifiWakeup,
    >,
    pub memory: ActorContext<Memory>,
    pub ld1: ActorContext<Ld1Actor>,
    pub ld2: ActorContext<Ld2Actor>,
    pub blinker1: ActorContext<Blinker1Actor>,
    pub blinker2: ActorContext<Blinker2Actor>,
    pub button: InterruptContext<ButtonInterrupt>,
    pub i2c: I2cPackage,
    pub hts221: Hts221Package,
    pub timer: TimerPackage,
}

impl Device for MyDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        let timer_addr = self.timer.mount((), supervisor);

        let spi_addr = self.spi.mount((), supervisor);

        self.wifi.mount((spi_addr, timer_addr), supervisor);
        self.memory.mount((), supervisor);
        let ld1_addr = self.ld1.mount((), supervisor);
        let ld2_addr = self.ld2.mount((), supervisor);
        let i2c_addr = self.i2c.mount((), supervisor);

        self.blinker1.mount((ld1_addr, timer_addr), supervisor);
        self.blinker2.mount((ld2_addr, timer_addr), supervisor);

        self.hts221.mount((config.event_bus, i2c_addr), supervisor);

        self.button.mount(config.event_bus, supervisor);
    }
}

impl EventHandler<ButtonEvent> for MyDevice {
    fn on_event(&'static self, message: ButtonEvent)
    where
        Self: Sized,
    {
        match message {
            ButtonEvent::Pressed => {
                log::info!("[{}] button pressed", ActorInfo::name());
                self.blinker1.address().adjust_delay(Milliseconds(100u32));
                self.memory.address().notify(Query);
            }
            ButtonEvent::Released => {
                log::info!("[{}] button released", ActorInfo::name());
                self.blinker1.address().adjust_delay(Milliseconds(500u32));
            }
        }
    }
}

impl EventHandler<SensorAcquisition<Celsius>> for MyDevice {
    fn on_event(&'static self, message: SensorAcquisition<Celsius>)
    where
        Self: Sized,
    {
        log::info!(
            "[event-bus] temperature={:.2} relative_humidity={:.2}",
            message.temperature.into_fahrenheit(),
            message.relative_humidity
        );
    }
}
