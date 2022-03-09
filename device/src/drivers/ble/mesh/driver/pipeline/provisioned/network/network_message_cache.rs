use crate::drivers::ble::mesh::pdu::network::CleartextNetworkPDU;

use crate::drivers::ble::mesh::address::UnicastAddress;
use uluru::LRUCache;

static mut LRU: Option<LRUCache<CacheEntry, 100>> = None;

#[derive(PartialEq)]
struct CacheEntry {
    seq: u32,
    src: UnicastAddress,
    iv_index: u16,
}

pub struct NetworkMessageCache {
    //lru: LRUCache<CacheEntry, 100>,
}

impl Default for NetworkMessageCache {
    fn default() -> Self {
        unsafe {
            LRU.replace(Default::default());
        }
        Self {
            //lru: Default::default(),
        }
    }
}

impl NetworkMessageCache {
    pub fn has_seen(&mut self, iv_index: u32, pdu: &CleartextNetworkPDU) -> bool {
        let entry = CacheEntry {
            seq: pdu.seq,
            src: pdu.src,
            iv_index: (iv_index & 0xFFFF) as u16,
        };
        unsafe {
            if let Some(lru) = LRU.as_mut() {
                if let None = lru.find(|e| *e == entry) {
                    lru.insert(entry);
                    false
                } else {
                    true
                }
            } else {
                false
            }
        }
    }
}
