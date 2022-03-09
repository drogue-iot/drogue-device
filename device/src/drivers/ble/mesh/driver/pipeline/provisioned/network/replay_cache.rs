use crate::drivers::ble::mesh::address::UnicastAddress;
use uluru::LRUCache;

#[derive(PartialEq)]
struct CacheEntry {
    seq: u32,
    src: UnicastAddress,
    iv_index: u16,
}

pub struct ReplayCache<const CACHE_SIZE: usize> {
    lru: LRUCache<CacheEntry, CACHE_SIZE>,
}

impl<const CACHE_SIZE: usize> ReplayCache<CACHE_SIZE> {
    pub fn new() -> Self {
        Self {
            lru: Default::default(),
        }
    }
    pub fn has_seen(&mut self, iv_index: u32, seq: u32, src: UnicastAddress) -> bool {
        let iv_index = (iv_index & 0xFFFF) as u16;

        if let Some(entry) = self.lru.find(|e| e.src == src) {
            if iv_index < entry.iv_index {
                true
            } else if iv_index == entry.iv_index {
                if seq <= entry.seq {
                    true
                } else {
                    entry.seq = seq;
                    false
                }
            } else {
                entry.iv_index = iv_index;
                entry.seq = seq;
                false
            }
        } else {
            self.lru.insert(CacheEntry { seq, src, iv_index });
            false
        }
    }
}
