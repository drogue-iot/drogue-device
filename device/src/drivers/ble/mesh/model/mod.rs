use core::marker::PhantomData;
use crate::drivers::ble::mesh::InsufficientBuffer;
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use heapless::Vec;

pub mod foundation;
pub mod generic;

#[derive(Eq, PartialEq)]
pub struct CompanyIdentifier([u8;4]);

#[derive(Eq, PartialEq)]
pub enum FoundationIdentifier {
    Configuration,
    Health,
}

#[derive(Eq, PartialEq)]
pub enum ModelIdentifier {
    Foundation(FoundationIdentifier),
    SIG(u16),
    Vendor(CompanyIdentifier, [u8;4]),
}

pub struct Sink<M:Message> {
    _marker: PhantomData<M>,
}

impl<M:Message> Sink<M> {
    pub async fn transmit(&mut self, message: M) {
        todo!()
    }
}

pub trait Message {
    fn opcode(&self) -> Opcode;
    fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer>;
}

pub enum HandlerError {
    Unhandled,
    NotConnected,
}

pub trait Model {
    const IDENTIFIER: ModelIdentifier;
    type MESSAGE: Message;

    fn parse(&self, opcode: Opcode, parameters: &[u8]) -> Result<Option<Self::MESSAGE>, ParseError>;
    //fn connect(&mut self, sink: Sink<Self::MESSAGE>);
    //fn handle(&mut self, message: &Self::MESSAGE) -> Result<(), HandlerError>;
}

pub trait State {
    type TYPE;
}

pub trait ReadableState<S:State> {
    fn read(&self) -> S::TYPE;
}

pub trait WriteableState<S:State> {
    fn write(&mut self, val: &S::TYPE);
}

