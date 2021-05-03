use super::ip::IpAddress;
use core::future::Future;
use heapless::String;

#[derive(Debug)]
pub enum Join {
    Open,
    Wpa {
        ssid: String<32>,
        password: String<32>,
    },
}

#[derive(Debug)]
pub enum JoinError {
    Unknown,
    InvalidSsid,
    InvalidPassword,
    UnableToAssociate,
}

pub trait WifiSupplicant {
    type JoinFuture<'m>: Future<Output = Result<IpAddress, JoinError>>
    where
        Self: 'm;
    fn join<'m>(&'m mut self, join: Join) -> Self::JoinFuture<'m>;
}
