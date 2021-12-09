#![no_std]
#![macro_use]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]

pub(crate) mod fmt;

use core::future::Future;
use core::marker::PhantomData;
use drogue_device::{
    actors::sensors::Temperature,
    actors::tcp::TcpActor,
    clients::http::*,
    domain::{
        temperature::{Celsius, TemperatureScale},
        SensorAcquisition,
    },
    drivers::dns::*,
    drogue,
    traits::button::Button,
    traits::{ip::*, sensors::temperature::TemperatureSensor},
    Actor, ActorContext, Address, Inbox, Package,
};
use embassy::executor::Spawner;
use embassy::traits::gpio::WaitForAnyEdge;
use embedded_hal::digital::v2::InputPin;
use heapless::String;
use serde::{Deserialize, Serialize};

#[cfg(feature = "tls")]
use drogue_tls::Aes128GcmSha256;

#[cfg(feature = "tls")]
use drogue_device::actors::net::TlsConnectionFactory;

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

pub struct App<B>
where
    B: TemperatureBoard + 'static,
{
    host: &'static str,
    port: u16,
    username: &'static str,
    password: &'static str,
    socket: PhantomData<&'static B>,
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

impl<B> Actor for App<B>
where
    B: TemperatureBoard + 'static,
{
    #[cfg(feature = "tls")]
    type Configuration = TlsConnectionFactory<'static, B::Network, Aes128GcmSha256, B::Rng, 1>;

    #[cfg(not(feature = "tls"))]
    type Configuration = Address<'static, B::Network>;

    type Message<'m>
    where
        B: 'm,
    = Command;

    type OnMountFuture<'m, M>
    where
        B: 'm,
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

static DNS: StaticDnsResolver<'static, 2> = StaticDnsResolver::new(&[
    DnsEntry::new("localhost", IpAddress::new_v4(127, 0, 0, 1)),
    DnsEntry::new(
        "http.sandbox.drogue.cloud",
        IpAddress::new_v4(95, 216, 224, 167),
    ),
]);

pub trait TemperatureBoard {
    type NetworkPackage: Package<Primary = Self::Network>;
    type Network: TcpActor;
    type TemperatureScale: TemperatureScale;
    type Sensor: TemperatureSensor<Self::TemperatureScale>;
    type SensorReadyIndicator: WaitForAnyEdge + InputPin;
    type SendTrigger: SendTrigger;
    #[cfg(feature = "tls")]
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
    network: B::NetworkPackage,
    app: ActorContext<'static, App<B>, 3>,
    trigger: ActorContext<'static, AppTrigger<B>>,
    sensor:
        ActorContext<'static, Temperature<B::SensorReadyIndicator, B::Sensor, B::TemperatureScale>>,
}

pub struct TemperatureBoardConfig<B>
where
    B: TemperatureBoard + 'static,
{
    pub network: B::NetworkPackage,
    pub sensor: B::Sensor,
    pub sensor_ready: B::SensorReadyIndicator,
    pub send_trigger: B::SendTrigger,
}

impl<B> TemperatureDevice<B>
where
    B: TemperatureBoard + 'static,
{
    pub fn new(config: TemperatureBoardConfig<B>) -> Self {
        Self {
            network: config.network,
            sensor: ActorContext::new(Temperature::new(config.sensor_ready, config.sensor)),
            trigger: ActorContext::new(AppTrigger {
                trigger: config.send_trigger,
            }),
            app: ActorContext::new(App::new(
                HOST,
                PORT.parse::<u16>().unwrap(),
                USERNAME.trim_end(),
                PASSWORD.trim_end(),
            )),
        }
    }

    #[cfg(feature = "tls")]
    pub async fn mount(
        &'static self,
        spawner: Spawner,
        config: <B::NetworkPackage as Package>::Configuration,
        rng: B::Rng,
    ) {
        static mut TLS_BUFFER: [u8; 16384] = [0; 16384];

        let network = self.network.mount(config, spawner);
        let network = TlsConnectionFactory::new(network, rng, [unsafe { &mut TLS_BUFFER }; 1]);
        let app = self.app.mount(network, spawner);
        let sensor = self.sensor.mount((), spawner);
        self.trigger.mount((sensor, app.into()), spawner);
    }

    #[cfg(not(feature = "tls"))]
    pub async fn mount(
        &'static self,
        spawner: Spawner,
        config: <B::NetworkPackage as Package>::Configuration,
    ) {
        let network = self.network.mount(config, spawner);
        let app = self.app.mount(network, spawner);
        let sensor = self.sensor.mount((), spawner);
        self.trigger.mount((sensor, app.into()), spawner);
    }
}

pub struct AppTrigger<B>
where
    B: TemperatureBoard + 'static,
{
    trigger: B::SendTrigger,
}

impl<B> Actor for AppTrigger<B>
where
    B: TemperatureBoard + 'static,
{
    type Configuration = (
        Address<'static, Temperature<B::SensorReadyIndicator, B::Sensor, B::TemperatureScale>>,
        Address<'static, App<B>>,
    );

    type OnMountFuture<'m, M>
    where
        Self: 'm,
        B: 'm,
        M: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        config: Self::Configuration,
        _: Address<'static, Self>,
        _: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        let (mut sensor, app) = config;
        async move {
            loop {
                self.trigger.wait().await;
                if let Ok(data) = sensor.temperature().await {
                    let data = TemperatureData {
                        geoloc: None,
                        temp: Some(data.temperature.raw_value()),
                        hum: Some(data.relative_humidity),
                    };
                    let _ = app.request(Command::Update(data)).unwrap().await;
                }
                let _ = app.request(Command::Send).unwrap().await;
            }
        }
    }
}

impl<B> SendTrigger for B
where
    B: Button + 'static,
{
    type TriggerFuture<'m>
    where
        B: 'm,
    = impl Future + 'm;
    fn wait<'m>(&'m mut self) -> Self::TriggerFuture<'m> {
        self.wait_released()
    }
}

pub struct TimeTrigger(pub embassy::time::Duration);
impl SendTrigger for TimeTrigger {
    type TriggerFuture<'m>
    where
        Self: 'm,
    = impl Future + 'm;
    fn wait<'m>(&'m mut self) -> Self::TriggerFuture<'m> {
        embassy::time::Timer::after(self.0)
    }
}

pub struct AlwaysReady;
impl WaitForAnyEdge for AlwaysReady {
    type Future<'m> = impl Future<Output = ()> + 'm;
    fn wait_for_any_edge<'m>(&'m mut self) -> Self::Future<'m> {
        async move {}
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
