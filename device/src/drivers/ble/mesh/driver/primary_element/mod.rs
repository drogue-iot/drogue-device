pub mod configuration_server;

use crate::drivers::ble::mesh::driver::node::{Node, Receiver, Transmitter};
use crate::drivers::ble::mesh::element::{Element, ElementError, ElementSink};
use crate::drivers::ble::mesh::model::Model;
use crate::drivers::ble::mesh::model::foundation::configuration::{
    BeaconHandler, ConfigurationServer, ConfigurationServerHandler,
};
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::storage::Storage;
use rand_core::{CryptoRng, RngCore};

pub struct PrimaryElement<TX, RX, S, R>
where
    TX: Transmitter,
    RX: Receiver,
    S: Storage,
    R: RngCore + CryptoRng,
{
    configuration_server: ConfigurationServer<Node<TX, RX, S, R>>,
}

impl<TX, RX, S, R> Element for PrimaryElement<TX, RX, S, R>
where
    TX: Transmitter,
    RX: Receiver,
    S: Storage,
    R: RngCore + CryptoRng,
{
    fn connect(&mut self, sink: ElementSink) {
        //self.configuration_server.connect()
        todo!()
    }

    fn handle(&mut self, opcode: Opcode, payload: &[u8]) -> Result<(), ElementError> {
        if let Ok(Some(message)) = self.configuration_server.parse(opcode, payload) {
            self.configuration_server.handle(&message);
        }
        Ok(())
    }
}
