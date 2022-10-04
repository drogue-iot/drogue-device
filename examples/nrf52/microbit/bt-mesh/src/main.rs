#![no_std]
#![no_main]
#![macro_use]
#![feature(type_alias_impl_trait)]

use btmesh_common::{InsufficientBuffer, ParseError};
use btmesh_device::{
    BluetoothMeshModel, BluetoothMeshModelContext, Control, InboundModelPayload, PublicationCadence,
};
use btmesh_macro::{device, element};
use btmesh_models::sensor::{
    PropertyId, SensorConfig, SensorData, SensorDescriptor, SensorMessage as SM,
    SensorServer as SS, SensorStatus,
};
use btmesh_nrf_softdevice::*;
use core::future::Future;
use embassy_executor::Spawner;
use embassy_futures::select::{select, Either};
use embassy_time::{Duration, Ticker, Timer};
use futures::StreamExt;
use heapless::Vec;
use nrf_softdevice::{temperature_celsius, Softdevice};

extern "C" {
    static __storage: u8;
}

use defmt_rtt as _;
use panic_probe as _;

// Application main entry point. The spawner can be used to start async tasks.
#[embassy_executor::main]
async fn main(_s: Spawner) {
    let _p = embassy_nrf::init(config());

    // Don't remove. Give flash some time before accessing
    Timer::after(Duration::from_millis(100)).await;

    // An instance of the Bluetooth Mesh stack
    let mut driver = Driver::new(
        "drogue",
        unsafe { &__storage as *const u8 as u32 },
        None,
        100,
        BluetoothMeshDriverConfig::default(),
    );

    // An instance of the sensor module implementing the SensorServer model.
    let sensor = Sensor::new(driver.softdevice());

    // An instance of our device with the models we'd like to expose.
    let mut device = Device::new(sensor);

    // Run the mesh stack
    let _ = driver.run(&mut device).await;
}

// A BluetoothMesh device with each field being a Bluetooth Mesh element.
#[device(cid = 0x0003, pid = 0x0001, vid = 0x0001)]
pub struct Device {
    front: Front,
}

// An element with multiple models.
#[element(location = "front")]
struct Front {
    sensor: Sensor,
}

impl Device {
    pub fn new(sensor: Sensor) -> Self {
        Self {
            front: Front { sensor },
        }
    }
}

// Application must run at a lower priority than softdevice. DO NOT CHANGE
fn config() -> embassy_nrf::config::Config {
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = embassy_nrf::interrupt::Priority::P2;
    config.time_interrupt_priority = embassy_nrf::interrupt::Priority::P2;
    config
}

// A sensor type implementing the SensorSetupServer model.
#[allow(dead_code)]
pub struct Sensor {
    // This field is required to access some peripherals that is also controlled by the radio driver
    sd: &'static Softdevice,
    ticker: Option<Ticker>,
}

impl Sensor {
    pub fn new(sd: &'static Softdevice) -> Self {
        Self { sd, ticker: None }
    }

    // Read the current on-chip temperature
    async fn read(&mut self) -> Result<SensorPayload, ()> {
        let temperature: i8 = temperature_celsius(self.sd).map_err(|_| ())?.to_num();

        Ok(SensorPayload {
            temperature: temperature * 2,
        })
    }

    // Process an inbound control message
    async fn process(&mut self, data: &InboundModelPayload<SensorMessage>) {
        match data {
            InboundModelPayload::Control(Control::PublicationCadence(cadence)) => match cadence {
                PublicationCadence::Periodic(cadence) => {
                    defmt::info!("Enabling sensor publish at {:?}", cadence.as_secs());
                    self.ticker.replace(Ticker::every(*cadence));
                }
                PublicationCadence::OnChange => {
                    defmt::info!("Sensor publish on change!");
                    self.ticker.take();
                }
                PublicationCadence::None => {
                    defmt::info!("Disabling sensor publish");
                    self.ticker.take();
                }
            },
            _ => {}
        }
    }
}

impl BluetoothMeshModel<SensorServer> for Sensor {
    type RunFuture<'f, C> = impl Future<Output=Result<(), ()>> + 'f
    where
        Self: 'f,
        C: BluetoothMeshModelContext<SensorServer> + 'f;

    fn run<'run, C: BluetoothMeshModelContext<SensorServer> + 'run>(
        &'run mut self,
        ctx: C,
    ) -> Self::RunFuture<'_, C> {
        async move {
            loop {
                if let Some(ticker) = self.ticker.as_mut() {
                    // When ticker is enabled, we emit sensor readings on each tick.
                    match select(ctx.receive(), ticker.next()).await {
                        Either::First(data) => self.process(&data).await,
                        Either::Second(_) => match self.read().await {
                            Ok(result) => {
                                defmt::info!("Read sensor data: {:?}", result);
                                let message = SensorMessage::Status(SensorStatus::new(result));
                                match ctx.publish(message).await {
                                    Ok(_) => {
                                        defmt::info!("Published sensor reading");
                                    }
                                    Err(e) => {
                                        defmt::warn!("Error publishing sensor reading: {:?}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                defmt::warn!("Error reading sensor data: {:?}", e);
                            }
                        },
                    }
                } else {
                    // When ticker is disabled, we wait for commands.
                    let m = ctx.receive().await;
                    self.process(&m).await;
                }
            }
        }
    }
}

#[derive(Debug, Clone, defmt::Format)]
pub struct MicrobitSensorConfig;

type SensorServer = SS<MicrobitSensorConfig, 1, 1>;
type SensorMessage = SM<MicrobitSensorConfig, 1, 1>;

#[derive(Debug, defmt::Format)]
pub struct SensorPayload {
    pub temperature: i8,
}

const PROP_TEMP: PropertyId = PropertyId(0x4F);

impl Default for SensorPayload {
    fn default() -> Self {
        Self { temperature: 0 }
    }
}

impl SensorData for SensorPayload {
    fn decode(&mut self, id: PropertyId, params: &[u8]) -> Result<(), ParseError> {
        if id == PROP_TEMP {
            self.temperature = params[0] as i8;
            Ok(())
        } else {
            Err(ParseError::InvalidValue)
        }
    }

    fn encode<const N: usize>(
        &self,
        property: PropertyId,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        if property == PROP_TEMP {
            xmit.extend_from_slice(&self.temperature.to_le_bytes())
                .map_err(|_| InsufficientBuffer)?;
        }
        Ok(())
    }
}

impl SensorConfig for MicrobitSensorConfig {
    type Data = SensorPayload;

    const DESCRIPTORS: &'static [SensorDescriptor] = &[SensorDescriptor::new(PROP_TEMP, 1)];
}
