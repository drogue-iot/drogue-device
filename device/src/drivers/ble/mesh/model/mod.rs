use defmt::Format;
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use heapless::Vec;

pub mod foundation;
pub mod generic;

#[derive(Copy, Clone, Eq, PartialEq, Format)]
pub struct CompanyIdentifier([u8; 4]);

#[derive(Copy, Clone, Eq, PartialEq, Format)]
pub enum FoundationIdentifier {
    Configuration,
    Health,
}

#[derive(Copy, Clone, Eq, PartialEq, Format)]
pub enum ModelIdentifier {
    Foundation(FoundationIdentifier),
    SIG(u16),
    Vendor(CompanyIdentifier, [u8; 4]),
}

pub trait Message {
    fn opcode(&self) -> Opcode;
    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer>;
}

pub enum HandlerError {
    Unhandled,
    NotConnected,
}

pub trait Model {
    const IDENTIFIER: ModelIdentifier;
    type MESSAGE: Message;

    fn parse(&self, opcode: Opcode, parameters: &[u8])
        -> Result<Option<Self::MESSAGE>, ParseError>;
}

pub trait State {
    type TYPE;
}

pub trait ReadableState<S: State> {
    fn read(&self) -> S::TYPE;
}

pub trait WriteableState<S: State> {
    fn write(&mut self, val: &S::TYPE);
}
