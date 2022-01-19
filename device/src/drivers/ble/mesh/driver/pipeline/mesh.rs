use crate::drivers::ble::mesh::bearer::advertising::PDU;
use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::PB_ADV;
use core::future::Future;

pub trait MeshContext {
    fn uuid(&self) -> Uuid;

    type TransmitFuture<'m>: Future<Output = Result<(), DeviceError>>
    where
        Self: 'm;

    fn transmit_pdu<'m>(&'m self, pdu: PDU) -> Self::TransmitFuture<'m>;
}

pub struct Mesh {}

pub enum MeshData {
    Provisioning(PDU),
}

impl Default for Mesh {
    fn default() -> Self {
        Self {}
    }
}

impl Mesh {
    pub async fn process_inbound<C: MeshContext>(
        &mut self,
        ctx: &C,
        data: &[u8],
    ) -> Result<Option<MeshData>, DeviceError> {
        if data.len() >= 2 {
            if data[1] == PB_ADV {
                Ok(Some(MeshData::Provisioning(
                    PDU::parse(data).map_err(|_| DeviceError::InvalidPacket)?,
                )))
            } else {
                Err(DeviceError::InvalidPacket)
            }
        } else {
            Ok(None)
        }
    }

    pub async fn process_outbound<C: MeshContext>(
        &mut self,
        ctx: &C,
        pdu: PDU,
    ) -> Result<(), DeviceError> {
        ctx.transmit_pdu(pdu).await
    }
}
