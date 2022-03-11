use core::future::Future;
use embedded_storage_async::nor_flash::{AsyncNorFlash, AsyncReadNorFlash};
/*
use p256::ecdh::SharedSecret;
use p256::elliptic_curve::group::GroupEncoding;
use p256::elliptic_curve::sec1::FromEncodedPoint;
use p256::elliptic_curve::subtle::CtOption;
use p256::elliptic_curve::Error;
use p256::{AffinePoint, EncodedPoint, SecretKey};
*/

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

/// Flash storage implementation
pub struct FlashStorage<F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    address: usize,
    flash: F,
}

impl<F> FlashStorage<F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    pub fn new(address: usize, flash: F) -> Self {
        Self { address, flash }
    }
}

impl<F> Storage for FlashStorage<F>
where
    F: AsyncNorFlash + AsyncReadNorFlash,
{
    type StoreFuture<'m> = impl Future<Output = Result<(), ()>>
    where
        Self: 'm;

    fn store<'m>(&'m mut self, keys: &'m Payload) -> Self::StoreFuture<'m> {
        async move {
            self.flash
                .erase(self.address as u32, self.address as u32 + 4096)
                .await
                .map_err(|_| ())?;
            self.flash
                .write(self.address as u32, &keys.payload)
                .await
                .map_err(|_| ())
        }
    }

    type RetrieveFuture<'m> = impl Future<Output = Result<Option<Payload>, ()>>
    where
        Self: 'm;

    fn retrieve<'m>(&'m mut self) -> Self::RetrieveFuture<'m> {
        async move {
            let mut payload = [0; 512];
            self.flash
                .read(self.address as u32, &mut payload)
                .await
                .map_err(|_| ())?;
            Ok(Some(Payload { payload }))
        }
    }
}
