#![no_std]
#![macro_use]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]

pub(crate) mod fmt;

use core::future::Future;
use core::marker::PhantomData;
use drogue_device::{
    traits,
    actors,
    actors::button::*, actors::net::*, clients::http::*, drivers::dns::*, traits::ip::*, Actor,
    Address, Inbox,
};
use drogue_device::{
    bsp::{App, AppBoard},
};
use heapless::String;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct GeoLocation {
    pub lon: f32,
    pub lat: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct TemperatureData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geoloc: Option<GeoLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hum: Option<f32>,
}

pub enum Command {
    Update(TemperatureData),
    Send,
}

pub struct TemperatureClient<C>
where
    C: ConnectionFactory + 'static,
{
    host: &'static str,
    port: u16,
    username: &'static str,
    password: &'static str,
    socket: PhantomData<&'static C>,
}

impl<C> TemperatureClient<C>
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

impl<C> Actor for TemperatureClient<C>
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
            let mut data: Option<TemperatureData> = None;
            loop {
                match inbox.next().await {
                    Some(mut m) => match m.message() {
                        Command::Update(d) => {
                            data.replace(d.clone());
                        }
                        Command::Send => {
                            if let Some(sensor_data) = data.as_ref() {
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
                                            // Pass on schema
                                            .path("/v1/foo?data_schema=urn:drogue:iot:temperature")
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
                            } else {
                                info!("Not temperature measurement received yet");
                            }
                        }
                    },
                    _ => {}
                }
            }
        }
    }
}

impl<C> FromButtonEvent<Command> for TemperatureClient<C>
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

static DNS: StaticDnsResolver<'static, 2> = StaticDnsResolver::new(&[
    DnsEntry::new("localhost", IpAddress::new_v4(127, 0, 0, 1)),
    DnsEntry::new(
        "http.sandbox.drogue.cloud",
        IpAddress::new_v4(95, 216, 224, 167),
    ),
]);

pub struct TemperatureConfiguration<B: TemperatureBoard> {
    pub send_button: B::SendButton,
    pub sensor: ActorContext<'static, B::TemperatureSensor>,
    pub network: ActorContext<'static, B::Network>
}

pub trait TemperatureBoard: AppBoard<TemperatureApp<Self>>
where
    Self: 'static,
{
    type Sensor: traits::sensors::TemperatureSensor<Celsius>;
    type Network: Actor + TcpActor<Self::Network>;
    type SendButton: traits::button::Button;
    type RccConfig;
}

impl<B: TemperatureBoard> App for TemperatureApp<B> {
    type Configuration = TemperatureConfiguration<B>;
    type Device = TemperatureDevice<B>;

    fn build(components: Self::Configuration) -> Self::Device {
        TemperatureDevice {
            client: ActorContext::new(HOST, PORT.parse::<u16>.unwrap(), USERNAME, PASSWORD),
            network: components.network,
            sensor: components.sensor,
            button: ActorContext::new(actors::button::Button::new(components.send_button)),
        }
    }
}


const USERNAME: &str = drogue::config!("http-username");
const PASSWORD: &str = drogue::config!("http-password");
const HOST: &str = drogue::config!("hostname");
const PORT: &str = drogue::config!("port");

pub struct TemperatureDevice<B: TemperatureBoard + 'static> {
    client: ActorContext<'static, TemperatureClient<B::ConnectionFactory>, 3>,
    button: ActorContext
    button: ActorContext<
        'static,
        actors::button::Button<B::SendButton, ButtonEventDispatcher<BlinkyApp<B>>>,
    >,
    i2c: ActorContext<'static, I2cPeripheral<I2cDriver>>,
    button: ActorContext<
        'static,
        Button<ExtiInput<'static, PC13>, ButtonEventDispatcher<App<ConnectionFactory>>>,
    >,
    sensor: ActorContext<
        'static,
        Sensor<ExtiInput<'static, PD15>, Address<'static, I2cPeripheral<I2cDriver>>>,
    >,
}
