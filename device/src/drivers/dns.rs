use crate::traits::{
    dns::{DnsError, DnsResolver},
    ip::IpAddress,
};
use core::future::Future;

pub struct DnsEntry<'a> {
    host: &'a str,
    ip: IpAddress,
}

impl<'a> DnsEntry<'a> {
    pub const fn new(host: &'a str, ip: IpAddress) -> Self {
        Self { host, ip }
    }
}

// A static DNS resolver that does not resolve duplicates
pub struct StaticDnsResolver<'a, const N: usize> {
    entries: &'a [DnsEntry<'a>; N],
}

impl<'a, const N: usize> StaticDnsResolver<'a, N> {
    pub const fn new(entries: &'a [DnsEntry<'a>; N]) -> Self {
        Self { entries }
    }
}
impl<'a, const N: usize> DnsResolver<1> for StaticDnsResolver<'a, N> {
    type ResolveFuture<'m>
    where
        'a: 'm,
    = impl Future<Output = Result<[IpAddress; 1], DnsError>> + 'm;
    fn resolve<'m>(&'m self, host: &'m str) -> Self::ResolveFuture<'m> {
        async move {
            for entry in self.entries.iter() {
                if entry.host == host {
                    return Ok([entry.ip; 1]);
                }
            }
            return Err(DnsError::NotFound);
        }
    }
}
