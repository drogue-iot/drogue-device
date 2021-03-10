use nucleo_f401re::{
    hal::{
        prelude::*,
        serial::{
            config::{Config, Parity, StopBits},
            Rx, Serial, Tx,
        },
    },
    pac::USART6,
};

use heapless::{
    consts::{U16, U2},
    i,
    spsc::Queue,
};

use drogue_esp8266::{adapter::Adapter, ingress::Ingress, initialize, protocol::Response};

type SerialTx = Tx<USART6>;
pub type SerialRx = Rx<USART6>;
pub type ESPAdapter = Adapter<'static, SerialTx>;

pub fn network_adapter(
    device: nucleo_f401re::pac::Peripherals,
) -> (ESPAdapter, Ingress<'static, SerialRx>) {
    let rcc = device.RCC.constrain();
    let clocks = rcc.cfgr.sysclk(84.mhz()).freeze();

    let gpioa = device.GPIOA.split();
    let gpioc = device.GPIOC.split();

    let pa11 = gpioa.pa11;
    let pa12 = gpioa.pa12;

    // SERIAL pins for USART6
    let tx_pin = pa11.into_alternate_af8();
    let rx_pin = pa12.into_alternate_af8();

    // enable pin
    let mut en = gpioc.pc10.into_push_pull_output();
    // reset pin
    let mut reset = gpioc.pc12.into_push_pull_output();

    let usart6 = device.USART6;

    let mut serial = Serial::usart6(
        usart6,
        (tx_pin, rx_pin),
        Config {
            baudrate: 115_200.bps(),
            parity: Parity::ParityNone,
            stopbits: StopBits::STOP1,
            ..Default::default()
        },
        clocks,
    )
    .unwrap();

    serial.listen(nucleo_f401re::hal::serial::Event::Rxne);
    let (tx, rx) = serial.split();

    static mut RESPONSE_QUEUE: Queue<Response, U2> = Queue(i::Queue::new());
    static mut NOTIFICATION_QUEUE: Queue<Response, U16> = Queue(i::Queue::new());

    initialize(
        tx,
        rx,
        &mut en,
        &mut reset,
        unsafe { &mut RESPONSE_QUEUE },
        unsafe { &mut NOTIFICATION_QUEUE },
    )
    .unwrap()
}
