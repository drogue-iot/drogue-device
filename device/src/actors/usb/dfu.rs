use crate::{
    actors::{
        dfu::{DfuCommand, DfuResponse, FirmwareManager},
        flash::SharedFlashHandle,
    },
    Actor, Address, Inbox,
};
use core::future::Future;
use core::str::FromStr;
use embassy::interrupt::InterruptExt;
use embassy::io::AsyncBufReadExt;
use embassy::io::AsyncWriteExt;
use embassy_nrf::{
    interrupt,
    peripherals::USBD,
    usb::{ClassSet1, ClassSet2, State, Usb, UsbBus, UsbSerial},
};
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};
use futures::pin_mut;
use nrf_usbd::Usbd;
use usb_device::bus::UsbBusAllocator;
use usb_device::device::{UsbDevice, UsbDeviceBuilder, UsbVidPid};

pub struct SerialUpdater<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash + 'static,
{
    bus: &'a UsbBusAllocator<Usbd<UsbBus<'a, USBD>>>,
    tx: &'a mut [u8],
    rx: &'a mut [u8],
    version: &'a [u8],
    dfu: Address<FirmwareManager<F>>,
}

impl<'a, F> SerialUpdater<'a, F>
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
            dfu,
            version,
        }
    }
}

impl<'a, F> Actor for SerialUpdater<'a, F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        M: 'm + Inbox<Self>;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
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

            let mut buf: [u8; 128] = [0; 128];
            let (mut reader, mut writer) = usb.as_ref().take_serial_0();

            info!("Starting serial updater");
            loop {
                if let Ok(c) = reader.read_byte().await {
                    match c {
                        1 => {
                            if let Ok(_) = writer.write_all(&[self.version.len() as u8]).await {
                                info!("Sent length");
                                if let Ok(_) = writer.write_all(&self.version).await {
                                    info!("Sent version");
                                }
                            }
                        }
                        2 => {
                            if let Ok(f) = self.dfu.request(DfuCommand::Start) {
                                if let DfuResponse::Ok = f.await {
                                    let _ = writer.write_all(&[1]).await;
                                } else {
                                    let _ = writer.write_all(&[2]).await;
                                }
                            } else {
                                let _ = writer.write_all(&[2]).await;
                            }
                        }
                        3 => {
                            let mut data = [0; 4];
                            info!("Write command");
                            if let Ok(_) = reader.read_exact(&mut data).await {
                                let offset = u32::from_le_bytes(data);
                                info!("Write: offset {}", offset);
                                if let Ok(_) = reader.read_exact(&mut data).await {
                                    let len = u32::from_le_bytes(data) as usize;
                                    info!("Write: len{}", offset);
                                    assert!(len <= 128);
                                    if let Ok(_) = reader.read_exact(&mut buf[..len]).await {
                                        if let Ok(f) =
                                            self.dfu.request(DfuCommand::WriteBlock(&buf[..len]))
                                        {
                                            if let DfuResponse::Ok = f.await {
                                                info!("Write block");
                                                let _ = writer.write_all(&[1]).await;
                                                info!("Done!");
                                            } else {
                                                let _ = writer.write_all(&[2]).await;
                                            }
                                        } else {
                                            let _ = writer.write_all(&[2]).await;
                                        }
                                    } else {
                                        let _ = writer.write_all(&[2]).await;
                                    }
                                } else {
                                    let _ = writer.write_all(&[2]).await;
                                }
                            } else {
                                let _ = writer.write_all(&[2]).await;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
