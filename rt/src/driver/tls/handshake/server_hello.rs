use heapless::{consts::*, Vec};

use crate::api::ip::tcp::{TcpSocket, TcpStack};
use crate::driver::tls::cipher_suites::CipherSuite;
use crate::driver::tls::extensions::server::ServerExtension;
use crate::driver::tls::handshake::Random;
use crate::driver::tls::parse_buffer::ParseBuffer;
use crate::driver::tls::TlsError;

#[derive(Debug)]
pub struct ServerHello {
    random: Random,
    legacy_session_id_echo: Vec<u8, U32>,
    cipher_suite: CipherSuite,
    extensions: Vec<ServerExtension, U16>,
}

impl ServerHello {
    pub async fn parse<T: TcpStack>(
        socket: &mut TcpSocket<T>,
        content_length: usize,
    ) -> Result<ServerHello, TlsError> {
        log::info!("parsing ServerHello");

        let mut buf = [0; 1024];
        let mut pos = 0;

        loop {
            pos += socket.read(&mut buf[pos..content_length as usize]).await?;
            if pos == content_length {
                break;
            }
        }

        let mut buf = ParseBuffer::new(&buf[0..content_length]);

        let version = buf.read_u16();

        let mut random = [0; 32];
        buf.fill(&mut random);

        let session_id_length = buf
            .read_u8()
            .map_err(|_| TlsError::InvalidSessionIdLength)?;

        //log::info!("sh 1");

        let mut session_id = Vec::<u8, U32>::new();
        buf.copy(&mut session_id, session_id_length as usize)
            .map_err(|_| TlsError::InvalidSessionIdLength)?;
        //log::info!("sh 2");

        let cipher_suite = buf.read_u16().map_err(|_| TlsError::InvalidCipherSuite)?;
        let cipher_suite = CipherSuite::of(cipher_suite).ok_or(TlsError::InvalidCipherSuite)?;

        ////log::info!("sh 3");
        // skip compression method, it's 0.
        buf.read_u8();

        //log::info!("sh 4");
        let extensions_length = buf
            .read_u16()
            .map_err(|_| TlsError::InvalidExtensionsLength)?;
        //log::info!("sh 5 {}", extensions_length);

        let extensions = ServerExtension::parse_vector(&mut buf)?;
        //log::info!("sh 6");

        log::info!("server random {:x?}", random);
        log::info!("server session-id {:x?}", session_id);
        log::info!("server cipher_suite {:x?}", cipher_suite);
        log::info!("server extensions {:?}", extensions);

        Ok(Self {
            random,
            legacy_session_id_echo: session_id,
            cipher_suite,
            extensions,
        })
    }
}
