use crate::drivers::ble::mesh::address::Address;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::access::AccessContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::pdu::access;
use crate::drivers::ble::mesh::pdu::access::{AccessMessage, AccessPayload, Config};

pub struct Beacon {}

impl Default for Beacon {
    fn default() -> Self {
        Self {}
    }
}

impl Beacon {
    pub async fn process_inbound<C: AccessContext>(
        &mut self,
        ctx: &C,
        message: &AccessMessage,
    ) -> Result<Option<AccessMessage>, DeviceError> {
        match message.payload {
            AccessPayload::Config(Config::Beacon(access::Beacon::Get)) => {
                defmt::info!("Beacon::Get");
                Ok(Some(AccessMessage {
                    network_key: message.network_key,
                    ivi: message.ivi,
                    nid: message.nid,
                    akf: message.akf,
                    aid: message.aid,
                    src: ctx
                        .primary_unicast_address()
                        .ok_or(DeviceError::NotProvisioned)?,
                    dst: Address::Unicast(message.src),
                    payload: AccessPayload::Config(Config::Beacon(access::Beacon::Status(true))),
                }))
            }
            AccessPayload::Config(Config::Beacon(access::Beacon::Set)) => Ok(None),
            AccessPayload::Config(Config::Beacon(access::Beacon::Status(_))) => Ok(None),
            _ => Ok(None),
        }
    }
}
