#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

mod app;

use app::*;

use defmt_rtt as _;
use panic_probe as _;

use core::cell::UnsafeCell;
use drogue_device::{
    actors::button::Button, drivers::lora::rak811::*, traits::lora::*, ActorContext, DeviceContext,
};
use embassy::util::Forever;
use embassy_nrf::{
    buffered_uarte::{BufferedUarte, State},
    gpio::{Input, Level, NoPin, Output, OutputDrive, Pull},
    gpiote::PortInput,
    interrupt,
    peripherals::{P0_14, P1_02, TIMER0, UARTE0},
    uarte, Peripherals,
};

const DEV_EUI: &str = include_str!(concat!(env!("OUT_DIR"), "/config/dev_eui.txt"));
const APP_EUI: &str = include_str!(concat!(env!("OUT_DIR"), "/config/app_eui.txt"));
const APP_KEY: &str = include_str!(concat!(env!("OUT_DIR"), "/config/app_key.txt"));

type UART = BufferedUarte<'static, UARTE0, TIMER0>;
type RESET = Output<'static, P1_02>;

pub struct MyDevice {
    driver: UnsafeCell<Rak811Driver>,
    modem: ActorContext<'static, Rak811ModemActor<'static, UART, RESET>>,
    app: ActorContext<'static, App<Rak811Controller<'static>>>,
    button: ActorContext<
        'static,
        Button<'static, PortInput<'static, P0_14>, App<Rak811Controller<'static>>>,
    >,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let button_port = PortInput::new(Input::new(p.P0_14, Pull::Up));

    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    static mut TX_BUFFER: [u8; 256] = [0u8; 256];
    static mut RX_BUFFER: [u8; 256] = [0u8; 256];
    static mut STATE: Forever<State<'static, UARTE0, TIMER0>> = Forever::new();

    let irq = interrupt::take!(UARTE0_UART0);
    let u = unsafe {
        let state = STATE.put(State::new());
        BufferedUarte::new(
            state,
            p.UARTE0,
            p.TIMER0,
            p.PPI_CH0,
            p.PPI_CH1,
            irq,
            p.P0_13,
            p.P0_01,
            NoPin,
            NoPin,
            config,
            &mut RX_BUFFER,
            &mut TX_BUFFER,
        )
    };

    let reset_pin = Output::new(p.P1_02, Level::High, OutputDrive::Standard);

    let join_mode = JoinMode::OTAA {
        dev_eui: DEV_EUI.trim_end().into(),
        app_eui: APP_EUI.trim_end().into(),
        app_key: APP_KEY.trim_end().into(),
    };

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN);

    DEVICE.configure(MyDevice {
        driver: UnsafeCell::new(Rak811Driver::new()),
        modem: ActorContext::new(Rak811ModemActor::new()),
        app: ActorContext::new(App::new(join_mode)),
        button: ActorContext::new(Button::new(button_port)),
    });

    DEVICE
        .mount(|device| async move {
            let (mut controller, modem) =
                unsafe { &mut *device.driver.get() }.initialize(u, reset_pin);
            device.modem.mount(modem, spawner);
            controller.configure(&config).await.unwrap();
            let app = device.app.mount(controller, spawner);
            device.button.mount(app, spawner);
        })
        .await;
}
