use drogue_device::{
    domain::time::duration::Milliseconds,
    driver::{
        button::{Button, ButtonEvent},
        lora::sx127x::*,
        spi::*,
        timer::Timer,
    },
    platform::cortex_m::stm32l0xx::{gpio::Pin, timer::HardwareTimer},
    prelude::*,
};
use stm32l0xx_hal::{
    delay::Delay,
    exti::GpioLine,
    gpio::{
        gpioa::{PA15, PA6, PA7},
        gpiob::{PB1, PB2, PB3, PB4},
        gpioc::PC0,
        Analog, Floating, Input, Output, PullUp, PushPull,
    },
    pac::{SPI1, TIM2},
    spi::{Error, Spi as HalSpi},
};

type SpiClk = PB3<Analog>;
type SpiMiso = PA6<Analog>;
type SpiMosi = PA7<Analog>;

type HardwareSpi = HalSpi<SPI1, (SpiClk, SpiMiso, SpiMosi)>;
type LoraPackage = Sx127x<
    AppTimer,
    HardwareSpi,
    PA15<Output<PushPull>>,
    PC0<Output<PushPull>>,
    PB1<Input<Floating>>,
    Delay,
    GpioLine,
    Error,
>;

pub type AppTimer = <Timer<HardwareTimer<TIM2>> as Package>::Primary;
use lora_common::*;

pub struct MyDevice {
    pub button: InterruptContext<Button<MyDevice, Pin<PB2<Input<PullUp>>, GpioLine>>>,
    pub timer: Timer<HardwareTimer<TIM2>>,
    pub lora: LoraPackage,
    pub app: ActorContext<App<<LoraPackage as Package>::Primary>>,
}

impl Device for MyDevice {
    fn mount(&'static self, config: DeviceConfiguration<Self>, supervisor: &mut Supervisor) {
        let timer = self.timer.mount((), supervisor);
        self.button.mount(config.event_bus, supervisor);
        let lora = self.lora.mount(timer, supervisor);
        self.app.mount(lora, supervisor);
    }
}

impl EventHandler<ButtonEvent> for MyDevice {
    fn on_event(&'static self, message: ButtonEvent) {
        match message {
            ButtonEvent::Pressed => {
                log::info!("[{}] button pressed", ActorInfo::name());
            }
            ButtonEvent::Released => {
                log::info!("[{}] button released", ActorInfo::name());
            }
        }
        self.app.address().notify(message);
    }
}
