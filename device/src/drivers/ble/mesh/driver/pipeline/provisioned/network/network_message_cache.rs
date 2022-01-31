use crate::drivers::ble::mesh::pdu::network::CleartextNetworkPDU;

pub struct NetworkMessageCache {}

impl Default for NetworkMessageCache {
    fn default() -> Self {
        Self {}
    }
}

impl NetworkMessageCache {
    pub fn has_seen(&mut self, _pdu: &CleartextNetworkPDU) -> bool {
        todo!()
    }
}
