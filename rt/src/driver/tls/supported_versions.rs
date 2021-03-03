use heapless::{consts::*, Vec};

pub type ProtocolVersion = [u8; 2];
pub type ProtocolVersions = Vec<ProtocolVersion, U16>;

pub const TLS13: ProtocolVersion = [0x03, 0x04];
