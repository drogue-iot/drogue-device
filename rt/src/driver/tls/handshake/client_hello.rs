use heapless::{consts::*, Vec};
use p256::ecdh::EphemeralSecret;
use p256::elliptic_curve::rand_core::{CryptoRng, RngCore};
use p256::EncodedPoint;

use crate::api::ip::tcp::{TcpError, TcpSocket, TcpStack};
use crate::driver::tls::config::Config;
use crate::driver::tls::content_types::ContentType;
use crate::driver::tls::extensions::ClientExtension;
use crate::driver::tls::handshake::{HandshakeType, Random, LEGACY_VERSION};
use crate::driver::tls::named_groups::NamedGroup;
use crate::driver::tls::signature_schemes::SignatureScheme;
use crate::driver::tls::supported_versions::{ProtocolVersion, TLS13};

pub struct ClientHello<'h, R>
where
    R: CryptoRng + RngCore + Copy,
{
    config: &'h Config<R>,
    random: Option<Random>,
    secret: Option<EphemeralSecret>,
}

impl<'h, R> ClientHello<'h, R>
where
    R: CryptoRng + RngCore + Copy,
{
    pub fn new(config: &'h Config<R>) -> Self {
        Self {
            config,
            random: None,
            secret: None,
        }
    }

    pub async fn transmit<S: TcpStack>(
        &mut self,
        socket: &mut TcpSocket<S>,
    ) -> Result<(), TcpError> {
        let mut random = [0; 32];
        let mut rng = self.config.rng;
        rng.fill_bytes(&mut random);
        self.random.replace(random);
        self.secret.replace(EphemeralSecret::random(rng));

        let public_key = EncodedPoint::from(self.secret.as_ref().unwrap().public_key());
        let public_key = public_key.as_ref();

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

        buf.extend_from_slice(&LEGACY_VERSION.to_be_bytes());
        buf.extend_from_slice(&self.random.unwrap());

        // session id (empty)
        buf.push(0);

        // cipher suites (2+)
        buf.extend_from_slice(&((self.config.cipher_suites.len() * 2) as u16).to_be_bytes());
        for c in self.config.cipher_suites.iter() {
            buf.extend_from_slice(&(*c as u16).to_be_bytes());
        }

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
        supported_signature_algorithms.extend(self.config.signature_schemes.iter());
        extensions.push(ClientExtension::SignatureAlgorithms {
            supported_signature_algorithms,
        });

        let mut supported_groups = Vec::<NamedGroup, U16>::new();
        supported_groups.extend(self.config.named_groups.iter());
        extensions.push(ClientExtension::SupportedGroups { supported_groups });

        let mut opaque = Vec::<u8, U128>::new();
        opaque.extend_from_slice(public_key);

        extensions.push(ClientExtension::KeyShare {
            group: NamedGroup::Secp256r1,
            opaque,
        });

        extensions.push(ClientExtension::MaxFragmentLength(
            self.config.max_fragment_length,
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
