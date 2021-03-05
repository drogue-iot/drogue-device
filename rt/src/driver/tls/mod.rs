use crate::api::ip::tcp::TcpError;

pub mod cipher_suites;
pub mod config;
pub mod content_types;
pub mod extensions;
pub mod handshake;
pub mod max_fragment_length;
pub mod named_groups;
pub mod parse_buffer;
pub mod record;
pub mod signature_schemes;
pub mod supported_versions;
pub mod tls_tcp_stack;

pub enum TlsError {
    Unimplemented,
    TcpError(TcpError),
    InvalidRecord,
    UnknownContentType,
    UnknownExtensionType,
    InvalidHandshake,
    InvalidCipherSuite,
    InvalidExtensionsLength,
    InvalidSessionIdLength,
    InvalidSupportedVersions,
    InvalidKeyShare,
}

impl From<TcpError> for TlsError {
    fn from(e: TcpError) -> Self {
        Self::TcpError(e)
    }
}
