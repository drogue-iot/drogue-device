#![no_std]
#![macro_use]
#![feature(type_alias_impl_trait)]
#![feature(generic_associated_types)]

pub(crate) mod fmt;

use core::future::Future;
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
use embedded_hal::digital::v2::InputPin;
use embedded_hal_async::digital::Wait;
use heapless::String;
use serde::{Deserialize, Serialize};

#[cfg(feature = "tls")]
use drogue_tls::Aes128GcmSha256;

#[cfg(feature = "tls")]
use drogue_device::actors::net::TlsConnectionFactory;

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
    connection_factory: ConnectionFactory<B>,
}

#[cfg(feature = "tls")]
type ConnectionFactory<B> = TlsConnectionFactory<
    'static,
    <B as TemperatureBoard>::Network,
    Aes128GcmSha256,
    <B as TemperatureBoard>::Rng,
    1,
>;

#[cfg(not(feature = "tls"))]
type ConnectionFactory<B> = Address<<B as TemperatureBoard>::Network>;

impl<B> App<B>
where
    B: TemperatureBoard + 'static,
{
    pub fn new(
        host: &'static str,
        port: u16,
        username: &'static str,
        password: &'static str,
        connection_factory: ConnectionFactory<B>,
    ) -> Self {
        Self {
            host,
            port,
            username,
            password,
            connection_factory,
        }
    }
}

impl<B> Actor for App<B>
where
    B: TemperatureBoard + 'static,
{
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
        _: Address<Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
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
                                    &mut self.connection_factory,
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
    network: B::NetworkPackage,
    app: ActorContext<App<B>, 3>,
    trigger: ActorContext<AppTrigger<B>>,
    sensor: ActorContext<Temperature<B::SensorReadyIndicator, B::Sensor, B::TemperatureScale>>,
}

pub struct TemperatureBoardConfig<B>
where
    B: TemperatureBoard + 'static,
{
    pub sensor: B::Sensor,
    pub sensor_ready: B::SensorReadyIndicator,
    pub send_trigger: B::SendTrigger,
    pub network_config: <B::NetworkPackage as Package>::Configuration,
}

impl<B> TemperatureDevice<B>
where
    B: TemperatureBoard + 'static,
{
    pub fn new(network: B::NetworkPackage) -> Self {
        Self {
            network,
            sensor: ActorContext::new(),
            trigger: ActorContext::new(),
            app: ActorContext::new(),
        }
    }

    pub async fn mount(
        &'static self,
        spawner: Spawner,
        _rng: B::Rng,
        config: TemperatureBoardConfig<B>,
    ) {
        let network = self.network.mount(config.network_config, spawner);
        #[cfg(feature = "tls")]
        let network = {
            static mut TLS_BUFFER: [u8; 16384] = [0; 16384];
            TlsConnectionFactory::new(network, _rng, [unsafe { &mut TLS_BUFFER }; 1])
        };

        let app = self.app.mount(
            spawner,
            App::new(
                HOST,
                PORT.parse::<u16>().unwrap(),
                USERNAME.trim_end(),
                PASSWORD.trim_end(),
                network,
            ),
        );
        let sensor = self.sensor.mount(
            spawner,
            Temperature::new(config.sensor_ready, config.sensor),
        );
        self.trigger.mount(
            spawner,
            AppTrigger::new(config.send_trigger, sensor, app.into()),
        );
    }
}

pub struct AppTrigger<B>
where
    B: TemperatureBoard + 'static,
{
    trigger: B::SendTrigger,
    sensor: Address<Temperature<B::SensorReadyIndicator, B::Sensor, B::TemperatureScale>>,
    app: Address<App<B>>,
}

impl<B> AppTrigger<B>
where
    B: TemperatureBoard + 'static,
{
    pub fn new(
        trigger: B::SendTrigger,
        sensor: Address<Temperature<B::SensorReadyIndicator, B::Sensor, B::TemperatureScale>>,
        app: Address<App<B>>,
    ) -> Self {
        Self {
            trigger,
            sensor,
            app,
        }
    }
}

impl<B> Actor for AppTrigger<B>
where
    B: TemperatureBoard + 'static,
{
    type OnMountFuture<'m, M>
    where
        Self: 'm,
        B: 'm,
        M: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(&'m mut self, _: Address<Self>, _: &'m mut M) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<Self> + 'm,
    {
        async move {
            loop {
                self.trigger.wait().await;
                trace!("Trigger activated! Requesting sensor data");
                if let Ok(data) = self.sensor.temperature().await {
                    let data = TemperatureData {
                        geoloc: None,
                        temp: Some(data.temperature.raw_value()),
                        hum: Some(data.relative_humidity),
                    };
                    trace!("Updating temperature data: {:?}", data);
                    let _ = self.app.request(Command::Update(data)).unwrap().await;
                }
                let _ = self.app.request(Command::Send).unwrap().await;
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

use core::convert::Infallible;
pub struct AlwaysReady;
impl embedded_hal_1::digital::ErrorType for AlwaysReady {
    type Error = Infallible;
}

impl Wait for AlwaysReady {
    type WaitForHighFuture<'a>
    where
        Self: 'a,
    = impl Future<Output = Result<(), Self::Error>> + 'a;

    fn wait_for_high<'a>(&'a mut self) -> Self::WaitForHighFuture<'a> {
        async move { Ok(()) }
    }

    type WaitForLowFuture<'a>
    where
        Self: 'a,
    = impl Future<Output = Result<(), Self::Error>> + 'a;

    fn wait_for_low<'a>(&'a mut self) -> Self::WaitForLowFuture<'a> {
        async move { Ok(()) }
    }

    type WaitForRisingEdgeFuture<'a>
    where
        Self: 'a,
    = impl Future<Output = Result<(), Self::Error>> + 'a;

    fn wait_for_rising_edge<'a>(&'a mut self) -> Self::WaitForRisingEdgeFuture<'a> {
        async move { Ok(()) }
    }

    type WaitForFallingEdgeFuture<'a>
    where
        Self: 'a,
    = impl Future<Output = Result<(), Self::Error>> + 'a;

    fn wait_for_falling_edge<'a>(&'a mut self) -> Self::WaitForFallingEdgeFuture<'a> {
        async move { Ok(()) }
    }

    type WaitForAnyEdgeFuture<'a>
    where
        Self: 'a,
    = impl Future<Output = Result<(), Self::Error>> + 'a;

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
