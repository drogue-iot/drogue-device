#[allow(unused_imports)]
use drogue_device::net::dns::{DnsEntry, StaticDnsResolver};
#[allow(unused_imports)]
use embedded_nal_async::{AddrType, Dns, IpAddr, Ipv4Addr, SocketAddr};

pub static DNS: StaticDnsResolver<'static, 2> = StaticDnsResolver::new(&[
    DnsEntry::new("localhost", IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
    DnsEntry::new(
        "http.sandbox.drogue.cloud",
        IpAddr::V4(Ipv4Addr::new(65, 108, 135, 161)),
    ),
]);
