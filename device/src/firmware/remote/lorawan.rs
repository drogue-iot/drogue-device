use core::future::Future;
use embedded_update::{Command, Status, UpdateService};
use serde::Serialize;

use crate::traits::lora::{LoraDriver, LoraError, QoS};

const MTU: usize = 256;
pub type Mutex = embassy_util::blocking_mutex::raw::NoopRawMutex;
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

#[cfg(feature = "defmt")]
impl defmt::Format for Error {
    fn format(&self, f: defmt::Formatter<'_>) {
        match self {
            Self::Network(e) => e.format(f),
            Self::Codec(e) => defmt::write!(f, "{}", defmt::Debug2Format(&e)),
            Self::Protocol => defmt::write!(f, "Protocol"),
        }
    }
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

            // If there is no status update, don't bother waiting for a response, it will be scheduled later so we need to wait for it.
            if status.update.is_none() {
                debug!("Sending initial status update");
                self.driver
                    // Using port 223 for firmware updates
                    .send(QoS::Confirmed, 223, &self.tx[..size])
                    .await
                    .map_err(|e| Error::Network(e))?;
                Ok(Command::new_wait(None, None))
            } else {
                debug!("Sending status update over lorawan link");
                let rx_len = self
                    .driver
                    // Using port 223 for firmware updates
                    .send_recv(QoS::Confirmed, 223, &self.tx[..size], &mut self.rx[..])
                    .await
                    .map_err(|e| Error::Network(e))?;
                if rx_len > 0 {
                    debug!("Received DFU command!");
                    let command: Command<'m> =
                        serde_cbor::de::from_mut_slice(&mut self.rx[..rx_len])
                            .map_err(|e| Error::Codec(e))?;
                    Ok(command)
                } else {
                    //debug!("Got RX len: {}, bytes: {:x}", rx_len, &self.rx[..rx_len]);
                    debug!("No command received, let's wait");
                    Ok(Command::new_wait(None, None))
                }
            }
        }
    }
}
