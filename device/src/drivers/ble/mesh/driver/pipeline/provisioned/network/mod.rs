use crate::drivers::ble::mesh::driver::pipeline::mesh::MeshContext;
use embassy::time::Instant;

pub mod authentication;
pub mod network_message_cache;
pub mod relay;
pub mod replay_cache;
pub mod transmit;

pub trait NetworkContext: MeshContext {
    fn network_deadline(&self, deadline: Option<Instant>);
}
