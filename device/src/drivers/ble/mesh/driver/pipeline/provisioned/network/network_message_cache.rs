use crate::drivers::ble::mesh::pdu::network::CleartextNetworkPDU;

use uluru::LRUCache;

#[derive(PartialEq)]
struct CacheEntry;

pub struct NetworkMessageCache {
    lru: LRUCache<CacheEntry, 100>,
}

impl Default for NetworkMessageCache {
    fn default() -> Self {
        Self {
            lru: Default::default(),
        }
    }
}

impl NetworkMessageCache {
    pub fn has_seen(&mut self, _pdu: &CleartextNetworkPDU) -> bool {
        let entry = CacheEntry;
        if let None = self.lru.find(|e| *e == entry) {
            self.lru.insert(entry);
            false
        } else {
            true
        }
    }
}
