use crate::drivers::ble::mesh::model::{ReadableState, State, WriteableState};

pub trait GenericOnOff {}

impl<T: GenericOnOff> State for T {
    type TYPE = bool;
}

impl<T: GenericOnOff> ReadableState<T> for T {
    fn read(&self) -> bool {
        todo!()
    }
}

impl<T: GenericOnOff> WriteableState<T> for T {
    fn write(&mut self, _val: &bool) {
        todo!()
    }
}

pub trait GenericOnOffServer {}

pub enum GenericOnOffMessage {
    Get,
    Set,
    SetUnacknowledged,
}

/*
impl<T:GenericOnOffServer> Model for T {
    const IDENTIFIER: ModelIdentifier = ModelIdentifier::SIG(0x1000);
    type MESSAGE = GenericOnOffMessage;

    fn parse(opcode: Opcode, parameters: &[u8]) -> Option<Self::MESSAGE> {
        todo!()
    }
}
 */
