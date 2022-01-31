use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::ProvisioningPDU;
use crate::drivers::ble::mesh::InsufficientBuffer;
use heapless::Vec;

pub struct InboundSegments {
    segments: Vec<Option<Vec<u8, 64>>, 32>,
}

impl InboundSegments {
    pub fn new(seg_n: u8, data: &Vec<u8, 64>) -> Result<Self, InsufficientBuffer> {
        let mut this = Self {
            segments: Vec::new(),
        };
        for _ in 0..seg_n + 1 {
            this.segments.push(None).map_err(|_| InsufficientBuffer)?;
        }
        let mut chunk = Vec::new();
        chunk
            .extend_from_slice(data)
            .map_err(|_| InsufficientBuffer)?;
        this.segments[0] = Some(chunk);
        Ok(this)
    }

    fn is_complete(&self) -> bool {
        self.segments.iter().all(|e| matches!(e, Some(_)))
    }

    pub(crate) fn receive(
        &mut self,
        segment_index: u8,
        data: &Vec<u8, 64>,
    ) -> Result<Option<ProvisioningPDU>, DeviceError> {
        if let None = self.segments[segment_index as usize] {
            let mut chunk = Vec::new();
            chunk
                .extend_from_slice(data)
                .map_err(|_| DeviceError::InsufficientBuffer)?;
            self.segments[segment_index as usize] = Some(chunk);
        }

        if self.is_complete() {
            let mut data: Vec<u8, 1024> = Vec::new();
            self.fill(&mut data)?;
            let pdu = ProvisioningPDU::parse(&*data)?;
            Ok(Some(pdu))
        } else {
            Ok(None)
        }
    }

    pub(crate) fn fill<const N: usize>(&self, dst: &mut Vec<u8, N>) -> Result<(), DeviceError> {
        for chunk in &self.segments {
            dst.extend_from_slice(&chunk.as_ref().ok_or(DeviceError::IncompleteTransaction)?)
                .map_err(|_| DeviceError::InsufficientBuffer)?
        }

        Ok(())
    }
}
