use crate::hal::uart::UartRx;
use crate::prelude::*;

use embedded_hal::serial::Read;

pub struct SerialRx<D, RX>
where
    D: Device + EventHandler<SerialData> + 'static,
    RX: Read<u8> + UartRx + 'static,
{
    rx: RX,
    bus: Option<Address<EventBus<D>>>,
}

impl<D, RX> SerialRx<D, RX>
where
    D: Device + EventHandler<SerialData> + 'static,
    RX: Read<u8> + UartRx + 'static,
{
    pub fn new(rx: RX) -> Self {
        Self { rx, bus: None }
    }
}

impl<D, RX> Actor for SerialRx<D, RX>
where
    D: Device + EventHandler<SerialData> + 'static,
    RX: Read<u8> + UartRx + 'static,
{
    type Configuration = Address<EventBus<D>>;
    fn on_mount(&mut self, me: Address<Self>, config: Self::Configuration) {
        self.bus.replace(config);
    }

    fn on_start(mut self) -> Completion<Self> {
        self.rx.enable_interrupt();
        Completion::immediate(self)
    }
}

impl<D, RX> Interrupt for SerialRx<D, RX>
where
    D: Device + EventHandler<SerialData> + 'static,
    RX: Read<u8> + UartRx + 'static,
{
    fn on_interrupt(&mut self) {
        if self.rx.check_interrupt() {
            let bus = self.bus.as_ref().unwrap();
            loop {
                match self.rx.read() {
                    Ok(b) => {
                        bus.publish(SerialData(b));
                    }
                    Err(nb::Error::WouldBlock) => {
                        break;
                    }
                    Err(e) => {
                        log::warn!("Error while reading");
                        break;
                    }
                }
            }
        }
        self.rx.clear_interrupt();
    }
}

#[derive(Clone)]
pub struct SerialData(pub u8);
