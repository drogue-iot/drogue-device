pub mod advertising {
    use crate::drivers::ble::mesh::generic_provisioning::{
        GenericProvisioningError, GenericProvisioningPDU,
    };
    use crate::drivers::ble::mesh::{InsufficientBuffer, PB_ADV};
    use heapless::Vec;

    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub struct AdvertisingPDU {
        pub link_id: u32,
        pub transaction_number: u8,
        pub pdu: GenericProvisioningPDU,
    }

    #[cfg_attr(feature = "defmt", derive(defmt::Format))]
    pub enum PBAdvError {
        InvalidSize,
        Generic(GenericProvisioningError),
    }

    impl AdvertisingPDU {
        pub fn parse(data: &[u8]) -> Result<AdvertisingPDU, PBAdvError> {
            if data.len() >= 8 {
                if data[1] != PB_ADV {
                    Err(PBAdvError::InvalidSize)
                } else {
                    let link_id = u32::from_be_bytes([data[2], data[3], data[4], data[5]]);
                    let transaction_number = data[6];

                    let pdu = GenericProvisioningPDU::parse(&data[7..])
                        .map_err(|e| PBAdvError::Generic(e))?;
                    Ok(AdvertisingPDU {
                        link_id,
                        transaction_number,
                        pdu,
                    })
                }
            } else {
                Err(PBAdvError::InvalidSize)
            }
        }

        pub fn emit<const N: usize>(
            &self,
            xmit: &mut Vec<u8, N>,
        ) -> Result<(), InsufficientBuffer> {
            xmit.push(0xFF).map_err(|_| InsufficientBuffer)?; // placeholder for size.
            xmit.push(PB_ADV).map_err(|_| InsufficientBuffer)?;
            xmit.extend_from_slice(&self.link_id.to_be_bytes())
                .map_err(|_| InsufficientBuffer)?;
            xmit.push(self.transaction_number)
                .map_err(|_| InsufficientBuffer)?;
            self.pdu.emit(xmit)?;
            xmit[0] = xmit.len() as u8 - 1;
            Ok(())
        }
    }
}
