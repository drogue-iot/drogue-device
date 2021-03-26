use crate::api::ip::tcp::TcpError;

pub mod application_data;
pub mod certificate_types;
pub mod change_cipher_spec;
pub mod cipher_suites;
pub mod config;
pub mod content_types;
pub mod crypto_engine;
pub mod extensions;
pub mod handshake;
pub mod key_schedule;
pub mod max_fragment_length;
pub mod named_groups;
pub mod parse_buffer;
pub mod record;
pub mod signature_schemes;
pub mod supported_versions;
pub mod tls_connection;
pub mod tls_tcp_stack;

#[derive(Debug, Copy, Clone)]
pub enum TlsError {
    Unimplemented,
    TcpError(TcpError),
    InvalidRecord,
    UnknownContentType,
    UnknownExtensionType,
    InvalidHandshake,
    InvalidCipherSuite,
    InvalidSignatureScheme,
    InvalidSignature,
    InvalidExtensionsLength,
    InvalidSessionIdLength,
    InvalidSupportedVersions,
    InvalidApplicationData,
    InvalidKeyShare,
    InvalidCertificate,
    UnableToInitializeCryptoEngine,
    CryptoError,
}

impl From<TcpError> for TlsError {
    fn from(e: TcpError) -> Self {
        Self::TcpError(e)
    }
}
