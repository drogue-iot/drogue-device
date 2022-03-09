use crate::drivers::ble::mesh::pdu::network::CleartextNetworkPDU;

use crate::drivers::ble::mesh::address::UnicastAddress;
use uluru::LRUCache;

#[derive(PartialEq)]
struct CacheEntry {
    seq: u32,
    src: UnicastAddress,
    iv_index: u16,
}

pub struct NetworkMessageCache<const CACHE_SIZE: usize> {
    lru: LRUCache<CacheEntry, CACHE_SIZE>,
}

impl<const CACHE_SIZE: usize> NetworkMessageCache<CACHE_SIZE> {
    pub fn new() -> Self {
        Self {
            lru: Default::default(),
        }
    }

    pub fn has_seen(&mut self, iv_index: u32, pdu: &CleartextNetworkPDU) -> bool {
        let entry = CacheEntry {
            seq: pdu.seq,
            src: pdu.src,
            iv_index: (iv_index & 0xFFFF) as u16,
        };
        if let None = self.lru.find(|e| *e == entry) {
            self.lru.insert(entry);
            false
        } else {
            true
        }
    }
}
