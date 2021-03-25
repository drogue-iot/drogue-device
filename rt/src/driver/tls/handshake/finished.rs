use crate::driver::tls::parse_buffer::ParseBuffer;
use crate::driver::tls::TlsError;
use core::fmt::{Debug, Formatter};
use digest::generic_array::GenericArray;
use digest::Digest;
use heapless::{consts::*, Vec};

pub struct Finished<D: Digest> {
    pub verify: GenericArray<u8, D::OutputSize>,
    pub hash: Option<GenericArray<u8, D::OutputSize>>,
}

impl<D: Digest> Debug for Finished<D> {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Finished")
            .field("verify", &self.hash)
            .finish()
    }
}

impl<D: Digest> Finished<D> {
    pub fn parse(buf: &mut ParseBuffer, len: u32) -> Result<Self, TlsError> {
        log::info!("finished len: {}", len);
        let mut verify = GenericArray::default();
        buf.fill(&mut verify);
        //let hash = GenericArray::from_slice()
        //let hash: Result<Vec<u8, _>, ()> = buf
        //.slice(len as usize)
        //.map_err(|_| TlsError::InvalidHandshake)?
        //.into();
        log::info!("hash {:?}", verify);
        //let hash = hash.map_err(|_| TlsError::InvalidHandshake)?;
        log::info!("hash ng {:?}", verify);
        Ok(Self { verify, hash: None })
    }
}
