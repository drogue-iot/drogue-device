use heapless::{consts::*, Vec};
use p256::ecdh::EphemeralSecret;
use p256::{EncodedPoint, NistP256};
use rand_core::{CryptoRng, RngCore};

use crate::api::ip::tcp::{TcpError, TcpSocket, TcpStack};
use crate::driver::tls::cipher_suites::CipherSuite;
//use p256::elliptic_curve::AffinePoint;
use crate::driver::tls::config::Config;
use crate::driver::tls::content_types::ContentType;
use crate::driver::tls::content_types::ContentType::Handshake;
use crate::driver::tls::extensions::ClientExtension;
use crate::driver::tls::extensions::ExtensionType::SupportedVersions;
use crate::driver::tls::handshake::certificate::Certificate;
use crate::driver::tls::handshake::certificate_verify::CertificateVerify;
use crate::driver::tls::handshake::client_hello::ClientHello;
use crate::driver::tls::handshake::encrypted_extensions::EncryptedExtensions;
use crate::driver::tls::handshake::finished::Finished;
use crate::driver::tls::handshake::server_hello::ServerHello;
use crate::driver::tls::max_fragment_length::MaxFragmentLength;
use crate::driver::tls::named_groups::NamedGroup;
use crate::driver::tls::parse_buffer::ParseBuffer;
use crate::driver::tls::signature_schemes::SignatureScheme;
use crate::driver::tls::supported_versions::{ProtocolVersion, TLS13};
use crate::driver::tls::TlsError;
use crate::driver::tls::TlsError::InvalidHandshake;
use sha2::Digest;

pub mod certificate;
pub mod certificate_verify;
pub mod client_hello;
pub mod encrypted_extensions;
pub mod finished;
pub mod server_hello;

const LEGACY_VERSION: u16 = 0x0303;

type Random = [u8; 32];

const HELLO_RETRY_REQUEST_RANDOM: [u8; 32] = [
    0xCF, 0x21, 0xAD, 0x74, 0xE5, 0x9A, 0x61, 0x11, 0xBE, 0x1D, 0x8C, 0x02, 0x1E, 0x65, 0xB8, 0x91,
    0xC2, 0xA2, 0x11, 0x16, 0x7A, 0xBB, 0x8C, 0x5E, 0x07, 0x9E, 0x09, 0xE2, 0xC8, 0xA8, 0x33, 0x9C,
];

pub enum HandshakeType {
    ClientHello = 1,
    ServerHello = 2,
    NewSessionTicket = 4,
    EndOfEarlyData = 5,
    EncryptedExtensions = 8,
    Certificate = 11,
    CertificateRequest = 13,
    CertificateVerify = 15,
    Finished = 20,
    KeyUpdate = 24,
    MessageHash = 254,
}

impl HandshakeType {
    pub fn of(num: u8) -> Option<Self> {
        log::info!("find handshake type of {}", num);
        match num {
            1 => Some(HandshakeType::ClientHello),
            2 => Some(HandshakeType::ServerHello),
            4 => Some(HandshakeType::NewSessionTicket),
            5 => Some(HandshakeType::EndOfEarlyData),
            8 => Some(HandshakeType::EncryptedExtensions),
            11 => Some(HandshakeType::Certificate),
            13 => Some(HandshakeType::CertificateRequest),
            15 => Some(HandshakeType::CertificateVerify),
            20 => Some(HandshakeType::Finished),
            24 => Some(HandshakeType::KeyUpdate),
            254 => Some(HandshakeType::MessageHash),
            _ => None,
        }
    }
}

pub enum ClientHandshake<'config, R>
where
    R: CryptoRng + RngCore + Copy,
{
    ClientHello(ClientHello<'config, R>),
}

#[derive(Debug)]
pub enum ServerHandshake {
    ServerHello(ServerHello),
    EncryptedExtensions(EncryptedExtensions),
    Certificate(Certificate),
    CertificateVerify(CertificateVerify),
    Finished(Finished),
}

impl ServerHandshake {
    pub async fn read<D: Digest, T: TcpStack>(
        socket: &mut TcpSocket<T>,
        len: u16,
        digest: &mut D,
    ) -> Result<Self, TlsError> {
        let mut header = [0; 4];
        let mut pos = 0;
        loop {
            pos += socket.read(&mut header).await?;
            if pos == header.len() {
                break;
            }
        }

        match HandshakeType::of(header[0]) {
            None => Err(TlsError::InvalidHandshake),
            Some(handshake_type) => {
                let length = u32::from_be_bytes([0, header[1], header[2], header[3]]);
                match handshake_type {
                    HandshakeType::ClientHello => Err(TlsError::Unimplemented),
                    HandshakeType::ServerHello => {
                        log::info!("hash [{:x?}]", &header);
                        digest.update(&header);
                        Ok(ServerHandshake::ServerHello(
                            ServerHello::read(socket, length as usize, digest).await?,
                        ))
                    }
                    HandshakeType::NewSessionTicket => Err(TlsError::Unimplemented),
                    HandshakeType::EndOfEarlyData => Err(TlsError::Unimplemented),
                    HandshakeType::EncryptedExtensions => Err(TlsError::Unimplemented),
                    HandshakeType::Certificate => Err(TlsError::Unimplemented),
                    HandshakeType::CertificateRequest => Err(TlsError::Unimplemented),
                    HandshakeType::CertificateVerify => Err(TlsError::Unimplemented),
                    HandshakeType::Finished => Err(TlsError::Unimplemented),
                    HandshakeType::KeyUpdate => Err(TlsError::Unimplemented),
                    HandshakeType::MessageHash => Err(TlsError::Unimplemented),
                }
            }
        }
    }

    pub fn parse(buf: &mut ParseBuffer) -> Result<Self, TlsError> {
        let handshake_type =
            HandshakeType::of(buf.read_u8().map_err(|_| TlsError::InvalidHandshake)?)
                .ok_or(TlsError::InvalidHandshake)?;

        let content_len = buf.read_u24().map_err(|_| TlsError::InvalidHandshake)?;

        match handshake_type {
            //HandshakeType::ClientHello => {}
            //HandshakeType::ServerHello => {}
            //HandshakeType::NewSessionTicket => {}
            //HandshakeType::EndOfEarlyData => {}
            HandshakeType::EncryptedExtensions => Ok(ServerHandshake::EncryptedExtensions(
                EncryptedExtensions::parse(buf)?,
            )),
            HandshakeType::Certificate => {
                Ok(ServerHandshake::Certificate(Certificate::parse(buf)?))
            }

            //HandshakeType::CertificateRequest => {}
            HandshakeType::CertificateVerify => Ok(ServerHandshake::CertificateVerify(
                CertificateVerify::parse(buf)?,
            )),
            HandshakeType::Finished => Ok(ServerHandshake::Finished(Finished::parse(
                buf,
                content_len,
            )?)),
            //HandshakeType::KeyUpdate => {}
            //HandshakeType::MessageHash => {}
            _ => Err(TlsError::Unimplemented),
        }
    }
}
