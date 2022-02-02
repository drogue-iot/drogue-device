use crate::drivers::ble::mesh::pdu::access::Opcode;

pub enum ElementError {
    NotConnected,
}

pub struct ElementSink {

}

pub trait Element {
    fn connect(&mut self, sink: ElementSink);
    fn handle(&mut self, opcode: Opcode, payload: &[u8]) -> Result<(), ElementError>;
}

pub struct ProvisionedElement<E:Element> {
    element: E,
}
