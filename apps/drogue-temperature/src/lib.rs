#![no_std]
#![macro_use]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]

pub(crate) mod fmt;

use core::future::Future;
use core::marker::PhantomData;
use drogue_device::{
    actors::net::*, clients::http::*, drivers::dns::*, traits::ip::*, Actor, Address, Inbox,
};
use heapless::String;
use serde::{Deserialize, Serialize};

//use drogue_device::domain::temperature::Temperature;

#[derive(Clone, Serialize, Deserialize)]
pub struct GeoLocation {
    pub lon: f32,
    pub lat: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TemperatureData {
    pub geoloc: Option<GeoLocation>,
    pub temp: Option<f32>,
    pub hum: Option<f32>,
}

pub enum Command {
    Send(TemperatureData),
}

pub struct App<C>
where
    C: ConnectionFactory + 'static,
{
    host: &'static str,
    port: u16,
    username: &'static str,
    password: &'static str,
    socket: PhantomData<&'static C>,
}

impl<C> App<C>
where
    C: ConnectionFactory + 'static,
{
    pub fn new(
        host: &'static str,
        port: u16,
        username: &'static str,
        password: &'static str,
    ) -> Self {
        Self {
            host,
            port,
            username,
            password,
            socket: PhantomData,
        }
    }
}

impl<C> Actor for App<C>
where
    C: ConnectionFactory + 'static,
{
    type Configuration = C;

    type Message<'m>
    where
        C: 'm,
    = Command;

    type OnMountFuture<'m, M>
    where
        C: 'm,
        M: 'm,
    = impl Future<Output = ()> + 'm;
    fn on_mount<'m, M>(
        &'m mut self,
        mut connection_factory: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            let mut counter: usize = 0;
            loop {
                match inbox.next().await {
                    Some(mut m) => match m.message() {
                        Command::Send(sensor_data) => {
                            info!("Sending temperature measurement number {}", counter);
                            counter += 1;
                            let mut client = HttpClient::new(
                                &mut connection_factory,
                                &DNS,
                                self.host,
                                self.port,
                                self.username,
                                self.password,
                            );

                            let tx: String<128> =
                                serde_json_core::ser::to_string(&sensor_data).unwrap();
                            let mut rx_buf = [0; 1024];
                            let response = client
                                .request(
                                    Request::post()
                                        .path("/v1/foo")
                                        .payload(tx.as_bytes())
                                        .content_type(ContentType::ApplicationJson),
                                    &mut rx_buf[..],
                                )
                                .await;
                            match response {
                                Ok(response) => {
                                    info!("Response status: {:?}", response.status);
                                    if let Some(payload) = response.payload {
                                        let s = core::str::from_utf8(payload).unwrap();
                                        trace!("Payload: {}", s);
                                    } else {
                                        trace!("No response body");
                                    }
                                }
                                Err(e) => {
                                    warn!("Error doing HTTP request: {:?}", e);
                                }
                            }
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}

static DNS: StaticDnsResolver<'static, 2> = StaticDnsResolver::new(&[
    DnsEntry::new("localhost", IpAddress::new_v4(127, 0, 0, 1)),
    DnsEntry::new(
        "http.sandbox.drogue.cloud",
        IpAddress::new_v4(95, 216, 224, 167),
    ),
]);
