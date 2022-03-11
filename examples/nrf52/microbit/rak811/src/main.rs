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
    actors::button::{Button, ButtonEventDispatcher},
    bsp::boards::nrf52::microbit::*,
    drivers::lora::rak811::*,
    drogue,
    traits::lora::*,
    ActorContext, Board, DeviceContext,
};
use embassy::util::Forever;
use embassy_nrf::{
    buffered_uarte::{BufferedUarte, State},
    gpio::{Level, Output, OutputDrive},
    interrupt,
    peripherals::{P1_02, TIMER0, UARTE0},
    uarte, Peripherals,
};

const DEV_EUI: &str = drogue::config!("dev-eui");
const APP_EUI: &str = drogue::config!("app-eui");
const APP_KEY: &str = drogue::config!("app-key");

type UART = BufferedUarte<'static, UARTE0, TIMER0>;
type RESET = Output<'static, P1_02>;

pub struct MyDevice {
    driver: UnsafeCell<Rak811Driver>,
    modem: ActorContext<Rak811ModemActor<'static, UART, RESET>>,
    app: ActorContext<App<Rak811Controller<'static>>>,
    button: ActorContext<Button<ButtonA, ButtonEventDispatcher<App<Rak811Controller<'static>>>>>,
}

static DEVICE: DeviceContext<MyDevice> = DeviceContext::new();

#[embassy::main]
async fn main(spawner: embassy::executor::Spawner, p: Peripherals) {
    let board = Microbit::new(p);

    let mut config = uarte::Config::default();
    config.parity = uarte::Parity::EXCLUDED;
    config.baudrate = uarte::Baudrate::BAUD115200;

    static mut TX_BUFFER: [u8; 256] = [0u8; 256];
    static mut RX_BUFFER: [u8; 256] = [0u8; 256];
    static mut STATE: Forever<State<'static, UARTE0, TIMER0>> = Forever::new();

    let irq = interrupt::take!(UARTE0_UART0);
    let u = unsafe {
        let state = STATE.put(State::new());
        BufferedUarte::new_without_flow_control(
            state,
            board.uarte0,
            board.timer0,
            board.ppi_ch0,
            board.ppi_ch1,
            irq,
            board.p0_13,
            board.p0_01,
            config,
            &mut RX_BUFFER,
            &mut TX_BUFFER,
        )
    };

    let reset_pin = Output::new(board.p1_02, Level::High, OutputDrive::Standard);

    let join_mode = JoinMode::OTAA {
        dev_eui: DEV_EUI.trim_end().into(),
        app_eui: APP_EUI.trim_end().into(),
        app_key: APP_KEY.trim_end().into(),
    };

    let config = LoraConfig::new()
        .region(LoraRegion::EU868)
        .lora_mode(LoraMode::WAN);

    let device = DEVICE.configure(MyDevice {
        driver: UnsafeCell::new(Rak811Driver::new()),
        modem: ActorContext::new(),
        app: ActorContext::new(),
        button: ActorContext::new(),
    });

    let (mut controller, modem) = unsafe { &mut *device.driver.get() }.initialize(u, reset_pin);
    device.modem.mount(spawner, Rak811ModemActor::new(modem));
    controller.configure(&config).await.unwrap();
    let app = device.app.mount(spawner, App::new(join_mode, controller));
    device
        .button
        .mount(spawner, Button::new(board.button_a, app.into()));
}
