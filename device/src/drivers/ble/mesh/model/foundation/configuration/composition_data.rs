use crate::drivers::ble::mesh::composition::Composition;
use crate::drivers::ble::mesh::model::{Message, ModelIdentifier};
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use crate::opcode;
use heapless::Vec;

opcode!( CONFIG_COMPOSITION_DATA_GET 0x80, 0x08 );
opcode!( CONFIG_COMPOSITION_DATA_STATUS 0x02 );

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum CompositionDataMessage {
    Get(u8),
    Status(CompositionStatus),
}

#[allow(unused)]
impl CompositionDataMessage {
    pub fn parse_get(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.len() == 1 {
            Ok(Self::Get(parameters[0]))
        } else {
            Err(ParseError::InvalidLength)
        }
    }
}

impl Message for CompositionDataMessage {
    fn opcode(&self) -> Opcode {
        match self {
            Self::Get(_) => CONFIG_COMPOSITION_DATA_GET,
            Self::Status(_) => CONFIG_COMPOSITION_DATA_STATUS,
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        match self {
            CompositionDataMessage::Get(page) => {
                xmit.push(*page).map_err(|_| InsufficientBuffer)?
            }
            CompositionDataMessage::Status(inner) => inner.emit_parameters(xmit)?,
        }
        Ok(())
    }
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct CompositionStatus {
    pub(crate) page: u8,
    pub(crate) data: Composition,
}

impl CompositionStatus {
    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.push(self.page).map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.data.cid.0.to_be_bytes())
            .map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.data.pid.0.to_be_bytes())
            .map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.data.vid.0.to_be_bytes())
            .map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.data.crpl.to_be_bytes())
            .map_err(|_| InsufficientBuffer)?;
        self.data.features.emit(xmit)?;
        for element in self.data.elements.iter() {
            xmit.extend_from_slice(&element.loc.0.to_be_bytes())
                .map_err(|_| InsufficientBuffer)?;
            let sig_models: Vec<_, 10> = element
                .models
                .iter()
                .filter(|e| matches!(e, ModelIdentifier::SIG(_)))
                .collect();
            let vendor_models: Vec<_, 10> = element
                .models
                .iter()
                .filter(|e| matches!(e, ModelIdentifier::Vendor(..)))
                .collect();

            xmit.push(sig_models.len() as u8)
                .map_err(|_| InsufficientBuffer)?;
            xmit.push(vendor_models.len() as u8)
                .map_err(|_| InsufficientBuffer)?;

            for model in sig_models.iter() {
                model.emit(xmit)?
            }

            for model in vendor_models.iter() {
                model.emit(xmit)?
            }
        }
        Ok(())
    }
}
