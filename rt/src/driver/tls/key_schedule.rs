use core::marker::PhantomData;
use digest::generic_array::ArrayLength;
use digest::{BlockInput, FixedOutput, Reset, Update};
use heapless::{consts::*, Vec};
use hkdf::Hkdf;
use sha2::digest::generic_array::{typenum::Unsigned, GenericArray};
use sha2::Digest;

pub struct KeySchedule<D, KeyLen, IvLen>
where
    D: Update + BlockInput + FixedOutput + Reset + Default + Clone,
    D::BlockSize: ArrayLength<u8>,
    D::OutputSize: ArrayLength<u8>,
    KeyLen: ArrayLength<u8>,
    IvLen: ArrayLength<u8>,
{
    secret: GenericArray<u8, D::OutputSize>,
    transcript_hash: D,
    hkdf: Option<Hkdf<D>>,
    client_write_iv: Option<GenericArray<u8, IvLen>>,
    client_write_key: Option<GenericArray<u8, KeyLen>>,
    server_write_iv: Option<GenericArray<u8, IvLen>>,
    server_write_key: Option<GenericArray<u8, KeyLen>>,
    read_counter: u64,
    write_counter: u64,
}

enum ContextType {
    None,
    TranscriptHash,
    EmptyHash,
}

impl<D, KeyLen, IvLen> KeySchedule<D, KeyLen, IvLen>
where
    D: Update + BlockInput + FixedOutput + Reset + Default + Clone,
    D::BlockSize: ArrayLength<u8>,
    D::OutputSize: ArrayLength<u8>,
    KeyLen: ArrayLength<u8>,
    IvLen: ArrayLength<u8>,
{
    pub fn new() -> Self {
        Self {
            secret: Self::zero(),
            transcript_hash: D::new(),
            hkdf: None,
            client_write_iv: None,
            client_write_key: None,
            server_write_iv: None,
            server_write_key: None,
            read_counter: 0,
            write_counter: 0,
        }
    }

    pub(crate) fn transcript_hash(&mut self) -> &mut D {
        &mut self.transcript_hash
    }

    pub(crate) fn increment_read_counter(&mut self) {
        self.read_counter += 1;
    }

    pub(crate) fn increment_write_counter(&mut self) {
        self.write_counter += 1;
    }

    pub(crate) fn get_server_nonce(&self) -> GenericArray<u8, IvLen> {
        log::info!(
            "seq {} server iv {:x?}",
            self.read_counter,
            self.server_write_iv.as_ref().unwrap()
        );
        self.get_nonce(self.read_counter, self.server_write_iv.as_ref().unwrap())
    }

    pub(crate) fn get_client_nonce(&self) -> GenericArray<u8, IvLen> {
        self.get_nonce(self.write_counter, self.client_write_iv.as_ref().unwrap())
    }

    fn get_nonce(&self, counter: u64, iv: &GenericArray<u8, IvLen>) -> GenericArray<u8, IvLen> {
        log::debug!("counter = {}", counter);
        let counter = Self::pad::<IvLen>(&counter.to_ne_bytes());

        log::debug!("counter = {:x?}", counter);
        log::debug!("iv = {:x?}", iv);

        let mut nonce = GenericArray::default();

        for (index, (l, r)) in iv[0..IvLen::to_usize()]
            .iter()
            .zip(counter.iter())
            .enumerate()
        {
            nonce[index] = l ^ r
        }

        log::debug!("nonce {:x?}", nonce);

        nonce
    }

    fn pad<N: ArrayLength<u8>>(input: &[u8]) -> GenericArray<u8, N> {
        log::info!("padding input = {:x?}", input);
        let mut padded = GenericArray::default();
        for (index, byte) in input.iter().rev().enumerate() {
            log::info!(
                "{} pad {}={:x?}",
                index,
                ((N::to_usize() - index) - 1),
                *byte
            );
            padded[(N::to_usize() - index) - 1] = *byte;
        }
        padded
    }

    fn zero() -> GenericArray<u8, D::OutputSize> {
        GenericArray::default()
    }

    fn derived(&mut self) {
        self.secret = self.derive_secret(b"derived", ContextType::EmptyHash);
    }

    pub fn initialize_early_secret(&mut self) {
        let (secret, hkdf) =
            Hkdf::<D>::extract(Some(self.secret.as_ref()), Self::zero().as_slice());
        self.hkdf.replace(hkdf);
        self.secret = secret;
        // no right-hand jaunts (yet)
        self.derived();
    }

    pub fn initialize_handshake_secret(&mut self, ikm: &[u8]) {
        let (secret, hkdf) = Hkdf::<D>::extract(Some(self.secret.as_ref()), ikm);
        self.secret = secret;
        self.hkdf.replace(hkdf);

        log::info!("handshake secret {:x?}", self.secret);

        let client_secret = self.derive_secret(b"c hs traffic", ContextType::TranscriptHash);
        log::info!("c hs secret {:x?}", client_secret);
        let server_secret = self.derive_secret(b"s hs traffic", ContextType::TranscriptHash);
        log::info!("s hs secret {:x?}", server_secret);

        //let client_write_hkdf = Hkdf::<D>::new(None, client_secret.as_slice());
        let client_write_hkdf = Hkdf::from_prk(client_secret.as_slice()).unwrap();
        self.client_write_iv.replace(self.hkdf_expand_label(
            &client_write_hkdf,
            &self.make_hkdf_label(b"iv", ContextType::None, IvLen::to_u16()),
        ));
        log::info!("client write IV: {:x?}", self.client_write_iv);
        self.client_write_key.replace(self.hkdf_expand_label(
            &client_write_hkdf,
            &self.make_hkdf_label(b"key", ContextType::None, KeyLen::to_u16()),
        ));

        //let server_write_hkdf = Hkdf::<D>::new(None, &server_secret);
        let server_write_hkdf = Hkdf::from_prk(&server_secret).unwrap();
        log::info!("generate server write IV using {:x?}", server_secret);
        self.server_write_iv.replace(self.hkdf_expand_label(
            &server_write_hkdf,
            &self.make_hkdf_label(b"iv", ContextType::None, IvLen::to_u16()),
        ));
        log::info!("server write IV: {:x?}", self.server_write_iv);
        log::info!("generate server write key");
        self.server_write_key.replace(self.hkdf_expand_label(
            &server_write_hkdf,
            &self.make_hkdf_label(b"key", ContextType::None, KeyLen::to_u16()),
        ));

        self.derived();
    }

    pub fn initialize_master_secret(&mut self) {
        let (secret, hkdf) =
            Hkdf::<D>::extract(Some(self.secret.as_ref()), Self::zero().as_slice());
        self.secret = secret;
        self.hkdf.replace(hkdf);

        log::info!("traffic secret {:x?}", self.secret);

        let client_secret = self.derive_secret(b"c hs traffic", ContextType::TranscriptHash);
        log::info!("c traffic secret {:x?}", client_secret);
        let server_secret = self.derive_secret(b"s hs traffic", ContextType::TranscriptHash);
        log::info!("s traffic secret {:x?}", server_secret);

        //let client_write_hkdf = Hkdf::<D>::new(None, client_secret.as_slice());
        let client_write_hkdf = Hkdf::from_prk(client_secret.as_slice()).unwrap();
        self.client_write_iv.replace(self.hkdf_expand_label(
            &client_write_hkdf,
            &self.make_hkdf_label(b"iv", ContextType::None, IvLen::to_u16()),
        ));
        self.client_write_key.replace(self.hkdf_expand_label(
            &client_write_hkdf,
            &self.make_hkdf_label(b"key", ContextType::None, KeyLen::to_u16()),
        ));

        //let server_write_hkdf = Hkdf::<D>::new(None, server_secret.as_slice());
        let server_write_hkdf = Hkdf::from_prk(server_secret.as_slice()).unwrap();
        self.server_write_iv.replace(self.hkdf_expand_label(
            &server_write_hkdf,
            &self.make_hkdf_label(b"iv", ContextType::None, IvLen::to_u16()),
        ));
        self.server_write_key.replace(self.hkdf_expand_label(
            &server_write_hkdf,
            &self.make_hkdf_label(b"key", ContextType::None, KeyLen::to_u16()),
        ));
    }

    fn derive_secret(
        &mut self,
        label: &[u8],
        context_type: ContextType,
    ) -> GenericArray<u8, D::OutputSize> {
        let label = self.make_hkdf_label(label, context_type, D::OutputSize::to_u16());
        self.hkdf_expand_label(self.hkdf.as_ref().unwrap(), &label)
    }

    pub fn hkdf_expand_label<N: ArrayLength<u8>>(
        &self,
        hkdf: &Hkdf<D>,
        label: &[u8],
    ) -> GenericArray<u8, N> {
        let mut okm: GenericArray<u8, N> = Default::default();
        log::info!("label {:x?}", label);
        hkdf.expand(label, &mut okm);
        log::info!("expand {:x?}", okm);
        okm
    }

    fn make_hkdf_label(&self, label: &[u8], context_type: ContextType, len: u16) -> Vec<u8, U512> {
        let mut hkdf_label = Vec::new();
        hkdf_label.extend_from_slice(&len.to_be_bytes());

        let label_len = 6 + label.len() as u8;
        hkdf_label.extend_from_slice(&(label_len as u8).to_be_bytes());
        hkdf_label.extend_from_slice(b"tls13 ");
        hkdf_label.extend_from_slice(label);

        match context_type {
            ContextType::None => {
                hkdf_label.push(0);
            }
            ContextType::TranscriptHash => {
                let context = self.transcript_hash.clone().finalize();
                hkdf_label.extend_from_slice(&(context.len() as u8).to_be_bytes());
                hkdf_label.extend_from_slice(&context);
            }
            ContextType::EmptyHash => {
                let context = D::new().chain(&[]).finalize();
                hkdf_label.extend_from_slice(&(context.len() as u8).to_be_bytes());
                hkdf_label.extend_from_slice(&context);
            }
        }
        hkdf_label
    }

    fn client_write_iv(&self) -> &GenericArray<u8, IvLen> {
        self.client_write_iv.as_ref().unwrap()
    }

    fn client_write_key(&self) -> &GenericArray<u8, KeyLen> {
        self.client_write_key.as_ref().unwrap()
    }

    fn server_write_iv(&self) -> &GenericArray<u8, IvLen> {
        self.server_write_iv.as_ref().unwrap()
    }

    pub(crate) fn server_write_key(&self) -> &GenericArray<u8, KeyLen> {
        self.server_write_key.as_ref().unwrap()
    }
}
