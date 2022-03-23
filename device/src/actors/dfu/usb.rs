use super::serial::{SerialError, SerialResponse, SerialUpdateProtocol, FRAME_SIZE};
use crate::{actors::dfu::FirmwareManager, Actor, Address, Inbox};
use core::future::Future;
use embassy::interrupt::InterruptExt;
use embassy::io::AsyncBufReadExt;
use embassy::io::AsyncWriteExt;
use embassy_nrf::{
    interrupt,
    peripherals::USBD,
    usb::{State, Usb, UsbBus, UsbSerial},
};
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};
use futures::pin_mut;
use nrf_usbd::Usbd;
use postcard::{from_bytes, to_slice};
use usb_device::bus::UsbBusAllocator;
use usb_device::device::{UsbDeviceBuilder, UsbVidPid};

pub struct UsbUpdater<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash + 'static,
{
    bus: &'a UsbBusAllocator<Usbd<UsbBus<'a, USBD>>>,
    tx: &'a mut [u8],
    rx: &'a mut [u8],
    protocol: SerialUpdateProtocol<'a, F>,
}

impl<'a, F> UsbUpdater<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    pub fn new(
        bus: &'a mut UsbBusAllocator<Usbd<UsbBus<'a, USBD>>>,
        tx: &'a mut [u8],
        rx: &'a mut [u8],
        dfu: Address<FirmwareManager<F>>,
        version: &'a [u8],
    ) -> Self {
        Self {
            bus,
            tx,
            rx,
            protocol: SerialUpdateProtocol::new(dfu, version),
        }
    }
}

impl<'a, F> Actor for UsbUpdater<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<Self>;

    fn on_mount<'m, M>(&'m mut self, _: Address<Self>, _: &'m mut M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            let serial = UsbSerial::new(self.bus, self.rx, self.tx);
            let device = UsbDeviceBuilder::new(self.bus, UsbVidPid(0x16c0, 0x27dd))
                .manufacturer("Red Hat")
                .product("Serial port")
                .serial_number("dr0gue")
                .device_class(0x02)
                .build();

            let irq = interrupt::take!(USBD);
            irq.set_priority(interrupt::Priority::P3);

            let mut state = State::new();
            let usb = unsafe { Usb::new(&mut state, device, serial, irq) };

            pin_mut!(usb);

            let (mut reader, mut writer) = usb.as_ref().take_serial_0();

            info!("Starting USB updater");
            let mut buf = [0; FRAME_SIZE];

            let response = self.protocol.initialize();
            if let Ok(_) = to_slice(&response, &mut buf) {
                let _ = writer.write_all(&buf).await;
            } else {
                warn!("Error initializing serial");
            }

            loop {
                if let Ok(_) = reader.read_exact(&mut buf[..]).await {
                    let response: Result<Option<SerialResponse>, SerialError> =
                        match from_bytes(&buf) {
                            Ok(command) => self.protocol.request(command).await,
                            Err(_e) => {
                                warn!("Error deserializing!");
                                Err(SerialError::Protocol)
                            }
                        };

                    if let Ok(_) = to_slice(&response, &mut buf) {
                        let _ = writer.write_all(&buf).await;
                    } else {
                        warn!("Error serializing response");
                    }
                }
            }
        }
    }
}
