use crate::drivers::ble::mesh::address::Address;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::lower::LowerContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::authentication::AuthenticationContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::network_message_cache::NetworkMessageCache;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::pdu::network::CleartextNetworkPDU;

pub trait RelayContext: LowerContext {
    fn is_local_unicast(&self, address: &Address) -> bool;
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
    pub async fn process_inbound<C: RelayContext>(
        &mut self,
        ctx: &C,
        pdu: &CleartextNetworkPDU,
    ) -> Result<Option<CleartextNetworkPDU>, DeviceError> {
        // only relay things that aren't exactly unicast to us.
        if !ctx.is_local_unicast(&pdu.dst) {
            // only relay if there's TTL remaining.
            if pdu.ttl >= 2
                && !self
                    .cache
                    .has_seen(ctx.iv_index().ok_or(DeviceError::NotProvisioned)?, pdu)
            {
                // decrease TTL and send a copy along.
                Ok(Some(CleartextNetworkPDU {
                    network_key: pdu.network_key,
                    ivi: pdu.ivi,
                    nid: pdu.nid,
                    ttl: pdu.ttl - 1,
                    seq: ctx.next_sequence().await?,
                    src: pdu.src,
                    dst: pdu.dst,
                    transport_pdu: pdu.transport_pdu.clone(),
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
