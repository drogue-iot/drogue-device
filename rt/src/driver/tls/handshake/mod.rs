use crate::driver::tls::cipher_suites::CipherSuite;
use crate::driver::tls::content_types::ContentType;
use crate::driver::tls::extensions::{ClientExtension, ServerExtension};

use crate::api::ip::tcp::{TcpError, TcpSocket, TcpStack};
use crate::driver::tls::extensions::supported_versions::{ProtocolVersion, TLS13};
use crate::driver::tls::extensions::ExtensionType::SupportedVersions;
use crate::driver::tls::max_fragment_length::MaxFragmentLength;
use crate::driver::tls::named_groups::NamedGroup;
use crate::driver::tls::signature_schemes::SignatureScheme;
use heapless::{consts::*, Vec};
use p256::ecdh::EphemeralSecret;
//use p256::elliptic_curve::AffinePoint;
use p256::EncodedPoint;
use rand_core::{CryptoRng, RngCore};

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

const LEGACY_VERSION: u16 = 0x0303;

type Random = [u8; 32];

pub struct ClientHello<R>
where
    R: CryptoRng + RngCore + Copy,
{
    rng: R,
    legacy_version: u16,
    random: Random,
    legacy_session_id: Vec<u8, U32>,
    cipher_suites: Vec<CipherSuite, U16>,
    legacy_compression_methods: [u8; 0],
    extensions: Vec<ClientExtension, U16>,
}

impl<R> ClientHello<R>
where
    R: CryptoRng + RngCore + Copy,
{
    pub fn new(rng: R, random: Random) -> Self {
        Self {
            rng,
            legacy_version: LEGACY_VERSION,
            random,
            legacy_session_id: Vec::new(),
            cipher_suites: Vec::new(),
            legacy_compression_methods: Default::default(),
            extensions: Vec::new(),
        }
    }

    pub async fn transmit<S: TcpStack>(&self, socket: &mut TcpSocket<S>) -> Result<(), TcpError> {
        let secret = EphemeralSecret::random(self.rng);
        let public_bytes = EncodedPoint::from(secret.public_key());
        let public_bytes = public_bytes.as_ref();
        log::info!("public: {:x?}", public_bytes);

        let mut buf: Vec<u8, U1024> = Vec::new();
        buf.push(ContentType::Handshake as u8);
        buf.extend_from_slice(&[0x03, 0x01]);

        let record_length_marker = buf.len();
        buf.push(0);
        buf.push(0);

        buf.push(HandshakeType::ClientHello as u8);

        let content_length_marker = buf.len();
        buf.push(0);
        buf.push(0);
        buf.push(0);

        buf.extend_from_slice(&self.legacy_version.to_be_bytes());
        buf.extend_from_slice(&self.random);

        // session id (empty)
        buf.push(0);

        // cipher suites (2+)
        buf.push(0);
        buf.push(2);
        //buf.extend_from_slice(&(CipherSuite::TlsChacha20Poly1305Sha256 as u16).to_be_bytes());
        buf.extend_from_slice(&(CipherSuite::TlsAes128GcmSha256 as u16).to_be_bytes());

        // compression methods, 1 byte of 0
        buf.push(1);
        buf.push(0);

        // extensions (1+)
        let mut extensions = Vec::<ClientExtension, U16>::new();
        let extension_length_marker = buf.len();
        buf.push(0);
        buf.push(0);

        let mut versions = Vec::<ProtocolVersion, U16>::new();
        versions.push(TLS13);
        extensions.push(ClientExtension::SupportedVersions { versions });

        let mut supported_signature_algorithms = Vec::<SignatureScheme, U16>::new();
        supported_signature_algorithms.push(SignatureScheme::RsaPssRsaeSha256);
        supported_signature_algorithms.push(SignatureScheme::RsaPssRsaeSha384);
        supported_signature_algorithms.push(SignatureScheme::RsaPssRsaeSha512);
        extensions.push(ClientExtension::SignatureAlgorithms {
            supported_signature_algorithms,
        });

        let mut supported_groups = Vec::<NamedGroup, U16>::new();
        supported_groups.push(NamedGroup::Secp256r1);
        extensions.push(ClientExtension::SupportedGroups { supported_groups });

        let mut opaque = Vec::<u8, U128>::new();
        opaque.extend_from_slice(public_bytes);

        extensions.push(ClientExtension::KeyShare {
            group: NamedGroup::Secp256r1,
            opaque,
        });

        extensions.push(ClientExtension::MaxFragmentLength(
            MaxFragmentLength::Bits10,
        ));

        // ----------------------------------------
        // ----------------------------------------

        for e in extensions {
            e.fill(&mut buf);
        }

        let extension_length = (buf.len() as u16 - extension_length_marker as u16) - 2;
        buf[extension_length_marker] = extension_length.to_be_bytes()[0];
        buf[extension_length_marker + 1] = extension_length.to_be_bytes()[1];

        let record_length = (buf.len() as u16 - record_length_marker as u16) - 2;

        buf[record_length_marker] = record_length.to_be_bytes()[0];
        buf[record_length_marker + 1] = record_length.to_be_bytes()[1];

        // u24, wtf?
        let content_length = (buf.len() as u32 - content_length_marker as u32) - 3;

        buf[content_length_marker] = content_length.to_be_bytes()[1];
        buf[content_length_marker + 1] = content_length.to_be_bytes()[2];
        buf[content_length_marker + 2] = content_length.to_be_bytes()[3];

        log::debug!("buf {:?}", buf);

        socket.write(buf.as_ref()).await?;

        Ok(())
    }
}

pub struct ServerHello {
    legacy_version: u16,
    random: Random,
    legacy_session_id_echo: Vec<u8, U32>,
    cipher_suite: CipherSuite,
    legacy_compression_method: u8,
    extensions: Vec<ServerExtension, U16>,
}

const HELLO_RETRY_REQUEST_RANDOM: [u8; 32] = [
    0xCF, 0x21, 0xAD, 0x74, 0xE5, 0x9A, 0x61, 0x11, 0xBE, 0x1D, 0x8C, 0x02, 0x1E, 0x65, 0xB8, 0x91,
    0xC2, 0xA2, 0x11, 0x16, 0x7A, 0xBB, 0x8C, 0x5E, 0x07, 0x9E, 0x09, 0xE2, 0xC8, 0xA8, 0x33, 0x9C,
];
