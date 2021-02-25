use drogue_device::{
    driver::{
        led::{Blinker, SimpleLED},
        timer::Timer,
    },
    hal::gpio::ActiveHigh,
    platform::cortex_m::nrf::timer::Timer as NrfTimer,
    prelude::*,
};
use hal::gpio::{Output, Pin, PushPull};
use nrf51_hal as hal;

type TimerPackage = Timer<NrfTimer<hal::pac::TIMER0>>;
type LedActor = SimpleLED<Pin<Output<PushPull>>, ActiveHigh>;

pub struct MyDevice {
    pub led: ActorContext<LedActor>,
    pub blinker: ActorContext<Blinker<LedActor, <TimerPackage as Package>::Primary>>,
    pub timer: TimerPackage,
}

impl Device for MyDevice {
    fn mount(&'static self, _: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        let timer_addr = self.timer.mount((), supervisor);
        let led_addr = self.led.mount((), supervisor);
        self.blinker.mount((led_addr, timer_addr), supervisor);
        log::info!("Started!");
    }
}
