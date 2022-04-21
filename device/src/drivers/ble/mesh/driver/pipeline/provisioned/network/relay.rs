use crate::drivers::ble::mesh::address::Address;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::lower::LowerContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::network_message_cache::NetworkMessageCache;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::driver::pipeline::mesh::NetworkRetransmitDetails;
use crate::drivers::ble::mesh::pdu::network::CleartextNetworkPDU;

pub trait RelayContext: LowerContext {
    fn is_relay_enabled(&self) -> bool;
    fn relay_retransmit(&self) -> NetworkRetransmitDetails;
}

pub struct Relay {
    cache: NetworkMessageCache,
}

impl Default for Relay {
    fn default() -> Self {
        Self {
            cache: Default::default(),
        }
    }
}

impl Relay {
    pub fn process_inbound<C: RelayContext>(
        &mut self,
        ctx: &C,
        pdu: &CleartextNetworkPDU,
    ) -> Result<Option<CleartextNetworkPDU>, DeviceError> {
        // if we are the src, drop.
        if ! ctx.is_relay_enabled() {
            return Ok(None);
        }
        if ctx.is_local_unicast(&Address::Unicast(pdu.src)) {
            return Ok(None);
        }

        // only relay things that aren't exactly unicast to us.
        if !ctx.is_local_unicast(&pdu.dst) {
            // only relay if there's TTL remaining.
            if pdu.ttl >= 2
                && !self
                    .cache
                    .has_seen(ctx.iv_index().ok_or(DeviceError::NotProvisioned)?, pdu)
            {
                info!("relay");
                // decrease TTL and send a copy along.
                Ok(Some(CleartextNetworkPDU {
                    //network_key: pdu.network_key,
                    //ivi: pdu.ivi,
                    //nid: pdu.nid,
                    //ttl: pdu.ttl - 1,
                    //seq: pdu.seq,
                    //src: pdu.src,
                    //dst: pdu.dst,
                    ttl: pdu.ttl - 1,
                    transport_pdu: pdu.transport_pdu.clone(),
                    ..*pdu
                }))
            } else {
                // don't relay, TTL expired
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}
