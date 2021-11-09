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

            // Attempt to parse host as IPv4 IP
            if let Ok(ip) = try_parse_ip(host) {
                return Ok([ip; 1]);
            }

            Err(DnsError::NotFound)
        }
    }
}

fn try_parse_ip(s: &str) -> Result<IpAddress, ()> {
    let mut octets: [u8; 4] = [0; 4];
    let mut s = s.split('.');
    if let Some(s) = s.next() {
        octets[0] = s.parse::<u8>().map_err(|_| ())?;
    }
    if let Some(s) = s.next() {
        octets[1] = s.parse::<u8>().map_err(|_| ())?;
    }
    if let Some(s) = s.next() {
        octets[2] = s.parse::<u8>().map_err(|_| ())?;
    }
    if let Some(s) = s.next() {
        octets[3] = s.parse::<u8>().map_err(|_| ())?;
    }

    Ok(IpAddress::new_v4(
        octets[0], octets[1], octets[2], octets[3],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ipv4() {
        let ip = try_parse_ip("192.168.1.2");
        assert!(ip.is_ok());
        assert_eq!(IpAddress::new_v4(192, 168, 1, 2), ip.unwrap());
    }
}
