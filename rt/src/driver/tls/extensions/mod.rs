use crate::driver::tls::extensions::supported_versions::{ProtocolVersion, ProtocolVersions};
use crate::driver::tls::signature_schemes::SignatureScheme;

use crate::driver::tls::max_fragment_length::MaxFragmentLength;
use crate::driver::tls::named_groups::NamedGroup;
use bbqueue::ArrayLength;
use heapless::{consts::*, Vec};

pub mod supported_versions;

pub enum ExtensionType {
    ServerName = 0,
    MaxFragmentLength = 1,
    StatusRequest = 5,
    SupportedGroups = 10,
    SignatureAlgorithms = 13,
    UseSrtp = 14,
    Heatbeat = 15,
    ApplicationLayerProtocolNegotiation = 16,
    SignedCertificateTimestamp = 18,
    ClientCertificateType = 19,
    ServerCertificateType = 20,
    Padding = 21,
    PreSharedKey = 41,
    EarlyData = 42,
    SupportedVersions = 43,
    Cookie = 44,
    PskKeyExchangeModes = 45,
    CertificateAuthorities = 47,
    OidFilters = 48,
    PostHandshakeAuth = 49,
    SignatureAlgorithmsCert = 50,
    KeyShare = 51,
}

pub enum ClientExtension {
    SupportedVersions {
        versions: ProtocolVersions,
    },
    SignatureAlgorithms {
        supported_signature_algorithms: Vec<SignatureScheme, U16>,
    },
    SupportedGroups {
        supported_groups: Vec<NamedGroup, U16>,
    },
    KeyShare {
        group: NamedGroup,
        opaque: Vec<u8, U128>,
    },
    SignatureAlgorithmsCert {
        supported_signature_algorithms: Vec<SignatureScheme, U16>,
    },
    MaxFragmentLength(MaxFragmentLength),
}

impl ClientExtension {
    pub fn extension_type(&self) -> [u8; 2] {
        match self {
            ClientExtension::SupportedVersions { .. } => ExtensionType::SupportedVersions as u16,
            ClientExtension::SignatureAlgorithms { .. } => {
                ExtensionType::SignatureAlgorithms as u16
            }
            ClientExtension::KeyShare { .. } => ExtensionType::KeyShare as u16,
            ClientExtension::SupportedGroups { .. } => ExtensionType::SupportedGroups as u16,
            ClientExtension::SignatureAlgorithmsCert { .. } => {
                ExtensionType::SignatureAlgorithmsCert as u16
            }
            ClientExtension::MaxFragmentLength(_) => ExtensionType::MaxFragmentLength as u16,
        }
        .to_be_bytes()
    }

    pub fn fill<N: ArrayLength<u8>>(&self, buf: &mut Vec<u8, N>) {
        buf.extend_from_slice(&self.extension_type());
        let extension_length_marker = buf.len();
        log::info!("marker at {}", extension_length_marker);
        buf.push(0);
        buf.push(0);

        match self {
            ClientExtension::SupportedVersions { versions } => {
                log::info!("supported versions ext");
                buf.push(versions.len() as u8 * 2);
                for v in versions {
                    buf.extend_from_slice(v);
                }
            }
            ClientExtension::SignatureAlgorithms {
                supported_signature_algorithms,
            } => {
                log::info!("supported sig algo ext");
                buf.extend_from_slice(
                    &(supported_signature_algorithms.len() as u16 * 2).to_be_bytes(),
                );

                for a in supported_signature_algorithms {
                    buf.extend_from_slice(&(*a as u16).to_be_bytes());
                }
            }
            ClientExtension::SignatureAlgorithmsCert {
                supported_signature_algorithms,
            } => {
                log::info!("supported sig algo cert ext");
                buf.extend_from_slice(
                    &(supported_signature_algorithms.len() as u16 * 2).to_be_bytes(),
                );

                for a in supported_signature_algorithms {
                    buf.extend_from_slice(&(*a as u16).to_be_bytes());
                }
            }
            ClientExtension::SupportedGroups { supported_groups } => {
                log::info!("supported groups ext");
                buf.extend_from_slice(&(supported_groups.len() as u16 * 2).to_be_bytes());

                for g in supported_groups {
                    buf.extend_from_slice(&(*g as u16).to_be_bytes());
                }
            }
            ClientExtension::KeyShare { group, opaque } => {
                log::info!("key_share ext");
                buf.extend_from_slice(&(2 + 2 as u16 + opaque.len() as u16).to_be_bytes());
                // one key-share
                buf.extend_from_slice(&(*group as u16).to_be_bytes());
                buf.extend_from_slice(&(opaque.len() as u16).to_be_bytes());
                buf.extend_from_slice(opaque.as_ref());
            }
            ClientExtension::MaxFragmentLength(len) => {
                log::info!("max fragment length");
                buf.push(*len as u8);
            }
        }

        log::info!("tail at {}", buf.len());
        let extension_length = (buf.len() as u16 - extension_length_marker as u16) - 2;
        log::info!("len: {}", extension_length);
        buf[extension_length_marker] = extension_length.to_be_bytes()[0];
        buf[extension_length_marker + 1] = extension_length.to_be_bytes()[1];
    }
}

pub enum ServerExtension {
    SupportedVersion { selected_version: ProtocolVersion },
}
