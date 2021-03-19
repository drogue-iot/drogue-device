use crate::driver::tls::cipher_suites::CipherSuite;
use crate::driver::tls::max_fragment_length::MaxFragmentLength;
use crate::driver::tls::named_groups::NamedGroup;
use crate::driver::tls::signature_schemes::SignatureScheme;
use heapless::{consts::*, Vec};
use rand_core::{CryptoRng, RngCore};

#[derive(Debug)]
pub struct Config<RNG>
where
    RNG: CryptoRng + RngCore,
{
    pub(crate) rng: RNG,
    pub(crate) cipher_suites: Vec<CipherSuite, U16>,
    pub(crate) signature_schemes: Vec<SignatureScheme, U16>,
    pub(crate) named_groups: Vec<NamedGroup, U16>,
    pub(crate) max_fragment_length: MaxFragmentLength,
}

impl<RNG> Config<RNG>
where
    RNG: CryptoRng + RngCore,
{
    pub fn new(rng: RNG) -> Self {
        let mut config = Self {
            rng,
            cipher_suites: Vec::new(),
            signature_schemes: Vec::new(),
            named_groups: Vec::new(),
            max_fragment_length: MaxFragmentLength::Bits10,
        };

        config.cipher_suites.push(CipherSuite::TlsAes128GcmSha256);

        config
            .signature_schemes
            .push(SignatureScheme::RsaPssRsaeSha256);
        config
            .signature_schemes
            .push(SignatureScheme::RsaPssRsaeSha384);
        config
            .signature_schemes
            .push(SignatureScheme::RsaPssRsaeSha512);

        config.named_groups.push(NamedGroup::Secp256r1);

        config
    }
}
