#![no_std]
#![macro_use]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]

pub(crate) mod fmt;

use core::convert::TryFrom;
use core::future::Future;
use drogue_device::{
    actors::sensors::Temperature,
    actors::transformer::Transformer,
    domain::{
        temperature::{Celsius, TemperatureScale},
        SensorAcquisition,
    },
    drivers::dns::*,
    drogue,
    traits::button::Button,
    traits::sensors::temperature::TemperatureSensor,
};
use ector::{Actor, ActorContext, Address, Inbox};
use embassy_executor::executor::Spawner;
use embedded_hal::digital::v2::InputPin;
use embedded_hal_async::digital::Wait;
use embedded_io::{Error, ErrorKind};
use embedded_nal_async::*;
use heapless::String;
use reqwless::{client::*, request::*};
use serde::{Deserialize, Serialize};

#[cfg(feature = "tls")]
use embedded_tls::{Aes128GcmSha256, NoClock, TlsConfig, TlsConnection, TlsContext};

#[derive(Clone, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct GeoLocation {
    pub lon: f32,
    pub lat: f32,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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

pub struct App<B>
where
    B: TemperatureBoard + 'static,
{
    host: &'static str,
    port: u16,
    username: &'static str,
    password: &'static str,
    network: B::Network,
    #[allow(dead_code)]
    rng: B::Rng,
}

impl<B> App<B>
where
    B: TemperatureBoard + 'static,
{
    pub fn new(
        host: &'static str,
        port: u16,
        username: &'static str,
        password: &'static str,
        network: B::Network,
        rng: B::Rng,
    ) -> Self {
        Self {
            host,
            port,
            username,
            password,
            network,
            rng,
        }
    }

    async fn send(&mut self, sensor_data: &TemperatureData) -> Result<(), ErrorKind> {
        debug!("Resolving {}:{}", self.host, self.port);
        let ip = DNS
            .get_host_by_name(self.host, AddrType::IPv4)
            .await
            .map_err(|_| ErrorKind::Other)?;

        #[cfg(feature = "tls")]
        let mut tls = [0; 16384];

        #[allow(unused_mut)]
        let mut connection = self
            .network
            .connect(SocketAddr::new(ip, self.port))
            .await
            .map_err(|e| e.kind())?;

        #[cfg(feature = "tls")]
        let mut connection = {
            let mut connection: TlsConnection<'_, _, Aes128GcmSha256> =
                TlsConnection::new(connection, &mut tls);
            connection
                .open::<_, NoClock, 1>(TlsContext::new(
                    &TlsConfig::new().with_server_name(self.host),
                    &mut self.rng,
                ))
                .await
                .map_err(|_| ErrorKind::Other)?;
            connection
        };

        debug!("Connected to {}:{}", self.host, self.port);

        let mut client = HttpClient::new(&mut connection, self.host);

        let tx: String<128> =
            serde_json_core::ser::to_string(&sensor_data).map_err(|_| ErrorKind::Other)?;
        let mut rx_buf = [0; 1024];
        let response = client
            .request(
                Request::post()
                    // Pass on schema
                    .path("/v1/foo?data_schema=urn:drogue:iot:temperature")
                    .basic_auth(self.username, self.password)
                    .payload(tx.as_bytes())
                    .content_type(ContentType::ApplicationJson)
                    .build(),
                &mut rx_buf[..],
            )
            .await;

        match response {
            Ok(response) => {
                info!("Response status: {:?}", response.status);
                if let Some(payload) = response.payload {
                    let s = core::str::from_utf8(payload).map_err(|_| ErrorKind::Other)?;
                    trace!("Payload: {}", s);
                } else {
                    trace!("No response body");
                }
                Ok(())
            }
            Err(e) => {
                warn!("Error doing HTTP request: {:?}", e);
                Err(ErrorKind::Other)
            }
        }
    }
}

impl<B> TryFrom<SensorAcquisition<B>> for Command
where
    B: TemperatureScale,
{
    type Error = Infallible;
    fn try_from(s: SensorAcquisition<B>) -> Result<Self, Self::Error> {
        Ok(Command::Update(TemperatureData {
            geoloc: None,
            temp: Some(s.temperature.raw_value()),
            hum: Some(s.relative_humidity),
        }))
    }
}

impl<B> Actor for App<B>
where
    B: TemperatureBoard + 'static,
{
    type Message<'m> = Command
    where
        B: 'm;

    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        B: 'm,
        M: 'm + Inbox<Command>;
    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Command>,
        mut inbox: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Command> + 'm,
    {
        async move {
            let mut counter: usize = 0;
            let mut data: Option<TemperatureData> = None;
            loop {
                match inbox.next().await {
                    Command::Update(d) => {
                        data.replace(d.clone());
                    }
                    Command::Send => {
                        if let Some(sensor_data) = data.as_ref() {
                            info!("Sending temperature measurement number {}", counter);
                            counter += 1;

                            match self.send(sensor_data).await {
                                Ok(_) => {
                                    info!("Temperature measurement sent");
                                }
                                Err(e) => {
                                    warn!("Error sending temperature measurement: {:?}", e);
                                }
                            }
                        } else {
                            info!("Not temperature measurement received yet");
                        }
                    }
                }
            }
        }
    }
}

static DNS: StaticDnsResolver<'static, 3> = StaticDnsResolver::new(&[
    DnsEntry::new("localhost", IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))),
    DnsEntry::new(
        "http.sandbox.drogue.cloud",
        IpAddr::V4(Ipv4Addr::new(65, 108, 135, 161)),
    ),
    DnsEntry::new(
        "http-endpoint-drogue-dev.apps.wonderful.iot-playground.org",
        IpAddr::V4(Ipv4Addr::new(65, 108, 135, 161)),
    ),
]);

pub trait TemperatureBoard {
    type Network: TcpConnect;
    type TemperatureScale: TemperatureScale;
    type Sensor: TemperatureSensor<Self::TemperatureScale>;
    type SensorReadyIndicator: Wait + InputPin;
    type SendTrigger: SendTrigger;
    type Rng: rand_core::RngCore + rand_core::CryptoRng;
}

pub trait SendTrigger {
    type TriggerFuture<'m>: Future
    where
        Self: 'm;
    fn wait<'m>(&'m mut self) -> Self::TriggerFuture<'m>;
}

pub struct TemperatureDevice<B>
where
    B: TemperatureBoard + 'static,
{
    app: ActorContext<App<B>, 3>,
    trigger: ActorContext<AppTrigger<B>>,
    sensor: ActorContext<Temperature<B::SensorReadyIndicator, B::Sensor, B::TemperatureScale>>,
    bridge: ActorContext<Transformer<SensorAcquisition<B::TemperatureScale>, Command>>,
}

pub struct TemperatureBoardConfig<B>
where
    B: TemperatureBoard + 'static,
{
    pub sensor: B::Sensor,
    pub sensor_ready: B::SensorReadyIndicator,
    pub send_trigger: B::SendTrigger,
    pub network: B::Network,
}

impl<B> TemperatureDevice<B>
where
    B: TemperatureBoard + 'static,
{
    pub fn new() -> Self {
        Self {
            sensor: ActorContext::new(),
            trigger: ActorContext::new(),
            app: ActorContext::new(),
            bridge: ActorContext::new(),
        }
    }

    pub async fn mount(
        &'static self,
        spawner: Spawner,
        rng: B::Rng,
        config: TemperatureBoardConfig<B>,
    ) {
        let network = config.network;

        let app = self.app.mount(
            spawner,
            App::new(
                HOST,
                PORT.parse::<u16>().unwrap(),
                USERNAME.trim_end(),
                PASSWORD.trim_end(),
                network,
                rng,
            ),
        );
        let bridge = self.bridge.mount(spawner, Transformer::new(app.clone()));
        self.sensor.mount(
            spawner,
            Temperature::new(config.sensor_ready, config.sensor, bridge),
        );

        self.trigger
            .mount(spawner, AppTrigger::new(config.send_trigger, app));
    }
}

pub struct AppTrigger<B>
where
    B: TemperatureBoard + 'static,
{
    trigger: B::SendTrigger,
    app: Address<Command>,
}

impl<B> AppTrigger<B>
where
    B: TemperatureBoard + 'static,
{
    pub fn new(trigger: B::SendTrigger, app: Address<Command>) -> Self {
        Self { trigger, app }
    }
}

impl<B> Actor for AppTrigger<B>
where
    B: TemperatureBoard + 'static,
{
    type Message<'m> = ();
    type OnMountFuture<'m, M> = impl Future<Output = ()> + 'm
    where
        Self: 'm,
        B: 'm,
        M: 'm + Inbox<Self::Message<'m>>;

    fn on_mount<'m, M>(
        &'m mut self,
        _: Address<Self::Message<'m>>,
        _: M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self::Message<'m>> + 'm,
    {
        async move {
            loop {
                self.trigger.wait().await;
                self.app.notify(Command::Send).await;
            }
        }
    }
}

impl<B> SendTrigger for B
where
    B: Button + 'static,
{
    type TriggerFuture<'m> = impl Future + 'm where B: 'm;
    fn wait<'m>(&'m mut self) -> Self::TriggerFuture<'m> {
        self.wait_released()
    }
}

pub struct TimeTrigger(pub embassy_executor::time::Duration);
impl SendTrigger for TimeTrigger {
    type TriggerFuture<'m> = impl Future + 'm where Self: 'm;
    fn wait<'m>(&'m mut self) -> Self::TriggerFuture<'m> {
        embassy_executor::time::Timer::after(self.0)
    }
}

use core::convert::Infallible;
pub struct AlwaysReady;
impl embedded_hal_1::digital::ErrorType for AlwaysReady {
    type Error = Infallible;
}

impl Wait for AlwaysReady {
    type WaitForHighFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a where Self: 'a;

    fn wait_for_high<'a>(&'a mut self) -> Self::WaitForHighFuture<'a> {
        async move { Ok(()) }
    }

    type WaitForLowFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a where Self: 'a;

    fn wait_for_low<'a>(&'a mut self) -> Self::WaitForLowFuture<'a> {
        async move { Ok(()) }
    }

    type WaitForRisingEdgeFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a where Self: 'a;

    fn wait_for_rising_edge<'a>(&'a mut self) -> Self::WaitForRisingEdgeFuture<'a> {
        async move { Ok(()) }
    }

    type WaitForFallingEdgeFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a where Self: 'a;

    fn wait_for_falling_edge<'a>(&'a mut self) -> Self::WaitForFallingEdgeFuture<'a> {
        async move { Ok(()) }
    }

    type WaitForAnyEdgeFuture<'a> = impl Future<Output = Result<(), Self::Error>> + 'a where Self: 'a;

    fn wait_for_any_edge<'a>(&'a mut self) -> Self::WaitForAnyEdgeFuture<'a> {
        async move { Ok(()) }
    }
}

impl InputPin for AlwaysReady {
    type Error = ();
    fn is_high(&self) -> Result<bool, Self::Error> {
        Ok(true)
    }

    fn is_low(&self) -> Result<bool, Self::Error> {
        Ok(false)
    }
}

pub struct FakeSensor(pub f32);

impl TemperatureSensor<Celsius> for FakeSensor {
    type Error = ();
    type CalibrateFuture<'m> = impl Future<Output = Result<(), Self::Error>> + 'm;
    fn calibrate<'m>(&'m mut self) -> Self::CalibrateFuture<'m> {
        async move { Ok(()) }
    }

    type ReadFuture<'m> =
        impl Future<Output = Result<SensorAcquisition<Celsius>, Self::Error>> + 'm;
    fn temperature<'m>(&'m mut self) -> Self::ReadFuture<'m> {
        async move {
            Ok(SensorAcquisition {
                relative_humidity: 0.0,
                temperature: drogue_device::domain::temperature::Temperature::new(self.0),
            })
        }
    }
}

const HOST: &str = drogue::config!("hostname");
const PORT: &str = drogue::config!("port");
const USERNAME: &str = drogue::config!("http-username");
const PASSWORD: &str = drogue::config!("http-password");
