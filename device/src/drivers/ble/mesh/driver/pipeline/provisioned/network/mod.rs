use crate::drivers::ble::mesh::driver::pipeline::mesh::MeshContext;
use embassy_time::Instant;

pub mod authentication;
pub mod network_message_cache;
#[cfg(feature = "ble-mesh-relay")]
pub mod relay;
pub mod replay_cache;
pub mod transmit;

pub trait NetworkContext: MeshContext {
    fn network_deadline(&self, deadline: Option<Instant>);
}
