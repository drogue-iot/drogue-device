use embassy::time::Instant;

pub mod authentication;
pub mod transmit;
pub mod network_message_cache;
pub mod relay;
pub mod replay_cache;

pub trait NetworkContext {
    fn network_deadline(&self, deadline: Option<Instant>);
}
