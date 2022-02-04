use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
use crate::drivers::ble::mesh::app::ApplicationKeyIdentifier;
use crate::drivers::ble::mesh::configuration_manager::NetworkKeyDetails;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use crate::drivers::ble::mesh::pdu::ParseError;
use core::convert::TryInto;
use defmt::Format;
use heapless::Vec;

#[derive(Format)]
pub enum UpperPDU {
    Control(UpperControl),
    Access(UpperAccess),
}

#[derive(Format)]
pub struct UpperControl {
    data: Vec<u8, 256>,
}

#[derive(Format)]
pub struct UpperAccess {
    pub(crate) network_key: NetworkKeyDetails,
    pub(crate) ivi: u8,
    pub(crate) nid: u8,
    pub(crate) akf: bool,
    pub(crate) aid: ApplicationKeyIdentifier,
    pub(crate) src: UnicastAddress,
    pub(crate) dst: Address,
    pub(crate) payload: Vec<u8, 380>,
}

impl TryInto<AccessMessage> for UpperPDU {
    type Error = ParseError;

    fn try_into(self) -> Result<AccessMessage, Self::Error> {
        match self {
            UpperPDU::Control(_) => Err(ParseError::InvalidPDUFormat),
            UpperPDU::Access(inner) => AccessMessage::parse(&inner),
        }
    }
}
