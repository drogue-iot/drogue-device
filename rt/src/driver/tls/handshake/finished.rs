use crate::driver::tls::parse_buffer::ParseBuffer;
use crate::driver::tls::TlsError;
use heapless::{consts::*, Vec};

#[derive(Debug)]
pub struct Finished {
    hash: Vec<u8, U32>,
}

impl Finished {
    pub fn parse(buf: &mut ParseBuffer, len: u32) -> Result<Self, TlsError> {
        log::info!("finished len: {}", len);
        let hash: Result<Vec<u8, _>, ()> = buf
            .slice(len as usize)
            .map_err(|_| TlsError::InvalidHandshake)?
            .into();
        log::info!("hash {:?}", hash);
        let hash = hash.map_err(|_| TlsError::InvalidHandshake)?;
        log::info!("hash ng {:?}", hash);
        Ok(Self { hash })
    }
}
