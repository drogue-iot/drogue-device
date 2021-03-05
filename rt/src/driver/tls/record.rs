use crate::api::ip::tcp::{TcpSocket, TcpStack};
use crate::driver::tls::content_types;
use crate::driver::tls::content_types::ContentType;
use crate::driver::tls::handshake::Handshake;
use crate::driver::tls::TlsError;

#[derive(Debug)]
pub enum Record {
    Handshake(Handshake),
    Alert,
    ApplicationData,
}

impl Record {
    pub async fn parse<T: TcpStack>(socket: &mut TcpSocket<T>) -> Result<Record, TlsError> {
        let mut header = [0; 5];
        let mut pos = 0;
        loop {
            pos += socket.read(&mut header[pos..]).await?;
            if pos == header.len() {
                break;
            }
        }

        match ContentType::of(header[0]) {
            None => Err(TlsError::InvalidRecord),
            Some(content_type) => {
                let content_length = u16::from_be_bytes([header[3], header[4]]);
                match content_type {
                    ContentType::Invalid => Err(TlsError::Unimplemented),
                    ContentType::ChangeCipherSpec => Err(TlsError::Unimplemented),
                    ContentType::Alert => Err(TlsError::Unimplemented),
                    ContentType::Handshake => Ok(Record::Handshake(
                        Handshake::parse(socket, content_length).await?,
                    )),
                    ContentType::ApplicationData => Err(TlsError::Unimplemented),
                }
            }
        }
    }
}
