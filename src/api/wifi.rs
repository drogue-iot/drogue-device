use crate::api::ip::{IpAddress, IpAddressV4};
use crate::prelude::*;
use heapless::{consts::*, ArrayLength, String};

#[derive(Debug)]
pub enum Join {
    Open,
    Wpa {
        ssid: String<U32>,
        password: String<U32>,
    },
}

#[derive(Debug)]
pub enum JoinError {
    Unknown,
    InvalidSsid,
    InvalidPassword,
    UnableToAssociate,
}

pub trait WifiSupplicant: Actor {
    fn join(self, join: Join) -> Response<Self, Result<IpAddress, JoinError>>;
}

impl<S> RequestHandler<Join> for S
where
    S: WifiSupplicant,
{
    type Response = Result<IpAddress, JoinError>;

    fn on_request(self, message: Join) -> Response<Self, Self::Response> {
        self.join(message)
    }
}

impl<S> Address<S>
where
    S: WifiSupplicant + 'static,
{
    pub async fn wifi_join(&self, join: Join) -> Result<IpAddress, JoinError> {
        self.request(join).await
    }
}
