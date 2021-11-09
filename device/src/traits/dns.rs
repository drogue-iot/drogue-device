use super::ip::IpAddress;
use core::future::Future;

// DNS errors that can be returned by resolver.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DnsError {
    NotFound,
}

/// Trait for asynchronuously resolving up to MAX_ENTRIES DNS entries for a given host
pub trait DnsResolver<const MAX_ENTRIES: usize> {
    type ResolveFuture<'m>: Future<Output = Result<[IpAddress; MAX_ENTRIES], DnsError>>
    where
        Self: 'm;

    /// Resolve a single host into MAX_ENTRIES ip addresses.
    fn resolve<'m>(&'m self, host: &'m str) -> Self::ResolveFuture<'m>;
}
