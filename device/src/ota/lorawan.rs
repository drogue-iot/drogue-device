use {
    embassy_lora::LoraTimer,
    embedded_update::{Command, Status, UpdateService},
    lorawan::default_crypto::DefaultFactory as Crypto,
    lorawan_device::async_device::{radio, Device, Timings},
    rand_core::RngCore,
    serde::Serialize,
};

const MTU: usize = 255;
pub type Mutex = embassy_sync::blocking_mutex::raw::NoopRawMutex;
pub type Payload = heapless::Vec<u8, MTU>;

pub struct LorawanService<R, RNG>
where
    R: radio::PhyRxTx + Timings,
    RNG: RngCore,
{
    device: Device<R, Crypto, LoraTimer, RNG>,
    tx: [u8; MTU],
    rx: [u8; MTU],
}

impl<R, RNG> LorawanService<R, RNG>
where
    R: radio::PhyRxTx + Timings,
    RNG: RngCore,
{
    pub fn new(device: Device<R, Crypto, LoraTimer, RNG>) -> Self {
        Self {
            device,
            tx: [0; MTU],
            rx: [0; MTU],
        }
    }
}

#[derive(Debug)]
pub enum Error {
    Network,
    Codec(serde_cbor::Error),
    Protocol,
}

#[cfg(feature = "defmt")]
impl defmt::Format for Error {
    fn format(&self, f: defmt::Formatter<'_>) {
        match self {
            Self::Network => defmt::write!(f, "Network"),
            Self::Codec(e) => defmt::write!(f, "{}", defmt::Debug2Format(&e)),
            Self::Protocol => defmt::write!(f, "Protocol"),
        }
    }
}

impl<R, RNG> UpdateService for LorawanService<R, RNG>
where
    R: radio::PhyRxTx + Timings,
    RNG: RngCore,
{
    type Error = Error;
    async fn request<'m>(&'m mut self, status: &'m Status<'m>) -> Result<Command<'m>, Self::Error> {
        let writer = serde_cbor::ser::SliceWrite::new(&mut self.tx[..]);
        let mut ser = serde_cbor::Serializer::new(writer).packed_format();
        status.serialize(&mut ser).map_err(|e| Error::Codec(e))?;
        let writer = ser.into_inner();
        let size = writer.bytes_written();

        // If there is no status update, don't bother waiting for a response, it will be scheduled later so we need to wait for it.
        if status.update.is_none() {
            debug!("Sending initial status update");
            self.device
                // Using port 223 for firmware updates
                .send(&self.tx[..size], 1, true)
                .await
                .map_err(|_e| Error::Network)?;
            Ok(Command::new_wait(None, None))
        } else {
            debug!("Sending status update over lorawan link");
            let rx_len = self
                .device
                // Using port 223 for firmware updates
                .send_recv(&self.tx[..size], &mut self.rx[..], 223, true)
                .await
                .map_err(|_e| Error::Network)?;
            if rx_len > 0 {
                debug!("Received DFU command!");
                let command: Command<'m> = serde_cbor::de::from_mut_slice(&mut self.rx[..rx_len])
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
