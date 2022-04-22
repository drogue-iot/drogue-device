use embassy::time::Instant;
use crate::drivers::ble::mesh::driver::pipeline::mesh::MeshContext;

pub mod authentication;
pub mod transmit;
pub mod network_message_cache;
pub mod relay;
pub mod replay_cache;

pub trait NetworkContext : MeshContext {
    fn network_deadline(&self, deadline: Option<Instant>);
}
