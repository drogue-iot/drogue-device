#![no_std]
#![macro_use]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]

pub(crate) mod fmt;

use core::fmt::Write;
use core::future::Future;
use core::marker::PhantomData;
use drogue_device::{
    actors::button::{ButtonEvent, FromButtonEvent},
    actors::net::*,
    actors::sensors::SensorMonitor,
    clients::http::*,
    domain::{temperature::Celsius, SensorAcquisition},
    drivers::dns::*,
    traits::ip::*,
    Actor, Address, Inbox,
};
use heapless::String;
//use drogue_device::domain::temperature::Temperature;

#[derive(Clone)]
pub struct GeoLocation {
    pub lon: f64,
    pub lat: f64,
}

#[derive(Clone)]
pub struct SensorData {
    pub data: SensorAcquisition<Celsius>,
    pub location: Option<GeoLocation>,
}

pub enum Command {
    Send,
    Update(SensorData),
}

impl<C> FromButtonEvent<Command> for App<C>
where
    C: ConnectionFactory + 'static,
{
    fn from(event: ButtonEvent) -> Option<Command> {
        match event {
            ButtonEvent::Pressed => None,
            ButtonEvent::Released => Some(Command::Send),
        }
    }
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
            let mut sensor_data: Option<SensorData> = None;
            loop {
                match inbox.next().await {
                    Some(mut m) => match m.message() {
                        Command::Update(t) => {
                            //trace!("Updating current app temperature measurement: {}", t);
                            sensor_data.replace(t.clone());
                        }
                        Command::Send => match &sensor_data {
                            Some(t) => {
                                info!("Sending temperature measurement");
                                let mut client = HttpClient::new(
                                    &mut connection_factory,
                                    &DNS,
                                    self.host,
                                    self.port,
                                    self.username,
                                    self.password,
                                );

                                let mut tx: String<256> = String::new();
                                if let Some(loc) = &t.location {
                                    write!(
                                        tx,
                                        "{{\"geoloc\": {{\"lat\": {}, \"lon\": {}}}, \"temp\": {}, \"hum\": {}}}",
                                        loc.lat,
                                        loc.lon,
                                        t.data.temperature.raw_value(), t.data.relative_humidity
                                    )
                                    .unwrap();
                                } else {
                                    write!(
                                        tx,
                                        "{{\"temp\": {}, \"hum\": {}}}",
                                        t.data.temperature.raw_value(),
                                        t.data.relative_humidity
                                    )
                                    .unwrap();
                                }
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
                                            info!("Payload: {}", s);
                                        } else {
                                            info!("No response body");
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Error doing HTTP request: {:?}", e);
                                    }
                                }
                            }
                            None => {
                                info!("No temperature value found, not sending measurement");
                            }
                        },
                    },
                    _ => {}
                }
            }
        }
    }
}

pub struct AppAddress<C: ConnectionFactory + 'static> {
    address: Address<'static, App<C>>,
}

impl<C: ConnectionFactory> From<Address<'static, App<C>>> for AppAddress<C> {
    fn from(address: Address<'static, App<C>>) -> Self {
        Self { address }
    }
}

impl<C: ConnectionFactory> SensorMonitor<Celsius> for AppAddress<C> {
    fn notify(&self, value: SensorAcquisition<Celsius>) {
        // Ignore channel full error
        let _ = self.address.notify(Command::Update(SensorData {
            data: value,
            location: None,
        }));
    }
}

static DNS: StaticDnsResolver<'static, 2> = StaticDnsResolver::new(&[
    DnsEntry::new("localhost", IpAddress::new_v4(127, 0, 0, 1)),
    DnsEntry::new(
        "http.sandbox.drogue.cloud",
        IpAddress::new_v4(95, 216, 224, 167),
    ),
]);
