use core::future::Future;
use p256::ecdh::SharedSecret;
use p256::elliptic_curve::group::GroupEncoding;
use p256::elliptic_curve::sec1::FromEncodedPoint;
use p256::elliptic_curve::subtle::CtOption;
use p256::elliptic_curve::Error;
use p256::{AffinePoint, EncodedPoint, SecretKey};

//pub struct Keys {
//private_key: [u8; 16],
//shared_secret: [u8; 16],
//}

/*
impl Keys {
    pub(crate) fn private_key(&self) -> Result<SecretKey, Error> {
        SecretKey::from_be_bytes(&self.private_key)
    }

    pub(crate) fn shared_secret(&self) -> Result<SharedSecret,Error> {
        let affine = AffinePoint::from_encoded_point(&EncodedPoint::from_bytes(&self.shared_secret)?);
        if affine.is_some().into() {
            Ok(
                SharedSecret::from(&affine.unwrap())
            )
        } else {
            Err(Error)
        }
    }
}
 */

#[repr(align(4))]
pub struct Payload {
    pub payload: [u8; 512],
}

pub trait Storage {
    type StoreFuture<'m>: Future<Output = Result<(), ()>>
    where
        Self: 'm;

    fn store<'m>(&'m mut self, payload: &'m Payload) -> Self::StoreFuture<'m>;

    type RetrieveFuture<'m>: Future<Output = Result<Option<Payload>, ()>>
    where
        Self: 'm;

    fn retrieve<'m>(&'m mut self) -> Self::RetrieveFuture<'m>;
}
