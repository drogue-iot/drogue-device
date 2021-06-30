use super::ip::IpAddress;
use core::future::Future;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Join<'a> {
    Open,
    Wpa { ssid: &'a str, password: &'a str },
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
    fn join<'m>(&'m mut self, join: Join<'m>) -> Self::JoinFuture<'m>;
}
