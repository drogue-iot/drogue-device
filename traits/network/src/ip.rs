use core::fmt::{Debug, Display, Formatter};

#[derive(Debug, Copy, Clone)]
pub enum IpAddress {
    V4(IpAddressV4),
}

impl IpAddress {
    pub const fn new_v4(a: u8, b: u8, c: u8, d: u8) -> Self {
        Self::V4(IpAddressV4(a, b, c, d))
    }
}

#[derive(Copy, Clone)]
pub struct IpAddressV4(u8, u8, u8, u8);

impl Display for IpAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            IpAddress::V4(addr) => {
                write!(f, "{}.{}.{}.{}", addr.0, addr.1, addr.2, addr.3)
            }
        }
    }
}

impl IpAddressV4 {
    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Self {
        IpAddressV4(a, b, c, d)
    }
}

impl Debug for IpAddressV4 {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0, self.1, self.2, self.3)
    }
}

impl Display for IpAddressV4 {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}.{}.{}.{}", self.0, self.1, self.2, self.3)
    }
}

#[derive(Debug)]
pub struct SocketAddress {
    ip: IpAddress,
    port: u16,
}

impl SocketAddress {
    pub fn new(ip: IpAddress, port: u16) -> Self {
        Self { ip, port }
    }

    pub fn ip(&self) -> IpAddress {
        self.ip
    }

    pub fn port(&self) -> u16 {
        self.port
    }
}

pub enum IpProtocol {
    Tcp,
    Udp,
}
