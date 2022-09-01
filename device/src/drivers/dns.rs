use core::future::Future;
use embedded_nal_async::*;
use heapless::String;

// DNS errors that can be returned by resolver.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DnsError {
    NotFound,
    ParseError,
}

pub struct DnsEntry<'a> {
    host: &'a str,
    ip: IpAddr,
}

impl<'a> DnsEntry<'a> {
    pub const fn new(host: &'a str, ip: IpAddr) -> Self {
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
impl<'a, const N: usize> Dns for StaticDnsResolver<'a, N> {
    type Error = DnsError;

    type GetHostByNameFuture<'m> = impl Future<Output = Result<IpAddr, DnsError>> + 'm where 'a: 'm;
    fn get_host_by_name<'m>(
        &'m self,
        host: &'m str,
        _addr_type: AddrType,
    ) -> Self::GetHostByNameFuture<'m> {
        async move {
            for entry in self.entries.iter() {
                if entry.host == host {
                    return Ok(entry.ip);
                }
            }

            // Attempt to parse host as IPv4 IP
            if let Ok(ip) = try_parse_ip(host) {
                return Ok(ip);
            }

            Err(DnsError::NotFound)
        }
    }

    type GetHostByAddressFuture<'m> = impl Future<Output = Result<String<256>, Self::Error>> + 'm
    where
        Self: 'm;
    fn get_host_by_address<'m>(&'m self, addr: IpAddr) -> Self::GetHostByAddressFuture<'m> {
        async move {
            for entry in self.entries.iter() {
                if entry.ip == addr {
                    return Ok(entry.host.into()); //.map_err(|_| DnsError::ParseError)?);
                }
            }
            Err(DnsError::NotFound)
        }
    }
}

fn try_parse_ip(s: &str) -> Result<IpAddr, ()> {
    let mut octets: [u8; 4] = [0; 4];
    for (i, item) in s.split('.').enumerate() {
        octets[i] = item.parse::<u8>().map_err(|_| ())?;
        if i == octets.len() - 1 {
            break;
        }
    }

    Ok(IpAddr::V4(Ipv4Addr::new(
        octets[0], octets[1], octets[2], octets[3],
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ipv4() {
        let ip = try_parse_ip("192.168.1.2");
        assert!(ip.is_ok());
        assert_eq!(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), ip.unwrap());

        let ip = try_parse_ip("192.168.1.2.2");
        assert!(ip.is_ok());
        assert_eq!(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)), ip.unwrap());
    }
}
