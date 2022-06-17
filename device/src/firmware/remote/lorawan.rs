use core::future::Future;
use embassy::channel::{Channel, DynamicReceiver, DynamicSender};
use embedded_nal_async::{SocketAddr, TcpClient};
use embedded_update::{Command, Status, UpdateService};
use serde::Serialize;

use crate::traits::lora::{LoraDriver, LoraError, QoS};

const MTU: usize = 256;
pub type Mutex = embassy::blocking_mutex::raw::NoopRawMutex;
pub type Payload = heapless::Vec<u8, MTU>;

pub struct LorawanService<D: LoraDriver> {
    driver: D,
    tx: [u8; MTU],
    rx: [u8; MTU],
}

impl<D> LorawanService<D>
where
    D: LoraDriver,
{
    pub fn new(driver: D) -> Self {
        Self {
            driver,
            tx: [0; MTU],
            rx: [0; MTU],
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Network(LoraError),
    Codec(serde_cbor::Error),
    Protocol,
}

impl<D> UpdateService for LorawanService<D>
where
    D: LoraDriver,
{
    type Error = Error;
    type RequestFuture<'m> = impl Future<Output = Result<Command<'m>, Self::Error>> + 'm where Self: 'm;
    fn request<'m>(&'m mut self, status: &'m Status<'m>) -> Self::RequestFuture<'m> {
        async move {
            let writer = serde_cbor::ser::SliceWrite::new(&mut self.tx[..]);
            let mut ser = serde_cbor::Serializer::new(writer).packed_format();
            status.serialize(&mut ser).map_err(|e| Error::Codec(e))?;
            let writer = ser.into_inner();
            let size = writer.bytes_written();

            let rx_len = self
                .driver
                .send_recv(QoS::Confirmed, 1, &self.tx[..size], &mut self.rx[..])
                .await
                .map_err(|e| Error::Network(e))?;
            if rx_len > 4 && &self.rx[..4] == b"dfu:" {
                let command: Command<'m> =
                    serde_cbor::de::from_mut_slice(&mut self.rx[4..rx_len - 4])
                        .map_err(|e| Error::Codec(e))?;
                Ok(command)
            } else {
                Err(Error::Protocol)
            }
        }
    }
}
