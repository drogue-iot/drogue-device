#![no_std]
#![no_main]
#![macro_use]
#![feature(type_alias_impl_trait)]

use btmesh_device::{BluetoothMeshModel, BluetoothMeshModelContext, InboundModelPayload};
use btmesh_macro::{device, element};
use btmesh_models::generic::onoff::{
    GenericOnOffClient, GenericOnOffMessage, GenericOnOffServer, Set as GenericOnOffSet,
};
use btmesh_nrf_softdevice::*;
use core::future::Future;
use embassy_executor::Spawner;
use embassy_nrf::gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin, Pull};
use embassy_time::{Duration, Timer};

extern "C" {
    static __storage: u8;
}

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "panic-reset")]
use panic_reset as _;

// Application main entry point. The spawner can be used to start async tasks.
#[embassy_executor::main]
async fn main(_s: Spawner) {
    let p = embassy_nrf::init(config());

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

    // An instance of our device with the models we'd like to expose.
    let mut device = Device::new(
        Output::new(p.P0_17.degrade(), Level::Low, OutputDrive::Standard),
        Input::new(p.P0_11.degrade(), Pull::Up),
    );

    // Run the mesh stack
    let _ = driver.run(&mut device).await;
}

// Application must run at a lower priority than softdevice. DO NOT CHANGE
fn config() -> embassy_nrf::config::Config {
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = embassy_nrf::interrupt::Priority::P2;
    config.time_interrupt_priority = embassy_nrf::interrupt::Priority::P2;
    config
}

// A BluetoothMesh device with each field being a Bluetooth Mesh element.
#[device(cid = 0x0003, pid = 0x0001, vid = 0x0001)]
pub struct Device {
    front: Front,
}

// An element with multiple models.
#[element(location = "front")]
struct Front {
    led: MyOnOffServerHandler,
    button: MyOnOffClientHandler,
}

impl Device {
    pub fn new(led: Output<'static, AnyPin>, button: Input<'static, AnyPin>) -> Self {
        Self {
            front: Front {
                led: MyOnOffServerHandler { led },
                button: MyOnOffClientHandler { button },
            },
        }
    }
}

struct MyOnOffServerHandler {
    led: Output<'static, AnyPin>,
}

impl BluetoothMeshModel<GenericOnOffServer> for MyOnOffServerHandler {
    type RunFuture<'f, C> = impl Future<Output=Result<(), ()>> + 'f
    where
        Self: 'f,
        C: BluetoothMeshModelContext<GenericOnOffServer> + 'f;

    fn run<'run, C: BluetoothMeshModelContext<GenericOnOffServer> + 'run>(
        &'run mut self,
        ctx: C,
    ) -> Self::RunFuture<'_, C> {
        async move {
            loop {
                let message = ctx.receive().await;
                if let InboundModelPayload::Message(message, _) = message {
                    match message {
                        GenericOnOffMessage::Get => {}
                        GenericOnOffMessage::Set(val) => {
                            if val.on_off == 1 {
                                self.led.set_high();
                            } else {
                                self.led.set_low();
                            }
                        }
                        GenericOnOffMessage::SetUnacknowledged(val) => {
                            if val.on_off == 1 {
                                self.led.set_high();
                            } else {
                                self.led.set_low();
                            }
                        }
                        GenericOnOffMessage::Status(_) => {
                            // not applicable
                        }
                    }
                }
            }
        }
    }
}

struct MyOnOffClientHandler {
    button: Input<'static, AnyPin>,
}

impl BluetoothMeshModel<GenericOnOffClient> for MyOnOffClientHandler {
    type RunFuture<'f, C> = impl Future<Output=Result<(), ()>> + 'f
    where
        Self: 'f,
        C: BluetoothMeshModelContext<GenericOnOffClient> + 'f;

    #[allow(clippy::await_holding_refcell_ref)]
    fn run<'run, C: BluetoothMeshModelContext<GenericOnOffClient> + 'run>(
        &'run mut self,
        ctx: C,
    ) -> Self::RunFuture<'_, C> {
        async move {
            let mut tid = 0;
            loop {
                self.button.wait_for_falling_edge().await;
                let message = GenericOnOffMessage::Set(GenericOnOffSet {
                    on_off: if self.button.is_low() { 1 } else { 0 },
                    tid,
                    transition_time: None,
                    delay: None,
                });

                // Publish event
                match ctx.publish(message).await {
                    Ok(_) => {
                        defmt::info!("Published button status ");
                    }
                    Err(e) => {
                        defmt::warn!("Error publishing button status: {:?}", e);
                    }
                }

                // Increase transaction id
                tid += 1;
            }
        }
    }
}
