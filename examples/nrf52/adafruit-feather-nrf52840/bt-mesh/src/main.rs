#![no_std]
#![no_main]
#![macro_use]
#![feature(type_alias_impl_trait)]
#![feature(async_fn_in_trait)]
#![allow(incomplete_features)]

use {
    adafruit_feather_nrf52::*,
    btmesh_device::{BluetoothMeshModel, BluetoothMeshModelContext, InboundModelPayload},
    btmesh_macro::{device, element},
    btmesh_models::generic::onoff::{
        GenericOnOffClient, GenericOnOffMessage, GenericOnOffServer, Set as GenericOnOffSet,
    },
    btmesh_nrf_softdevice::*,
    embassy_executor::Spawner,
    embassy_time::{Duration, Timer},
};

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
async fn main(s: Spawner) {
    let board = AdafruitFeatherNrf52::new(config());

    // Don't remove. Give flash some time before accessing
    Timer::after(Duration::from_millis(100)).await;

    // Watchdog will prevent bootloader from resetting. If your application hangs for more than 5 seconds
    // (depending on bootloader config), it will enter bootloader which may swap the application back.
    s.spawn(watchdog_task()).unwrap();

    // An instance of the Bluetooth Mesh stack
    let mut driver = Driver::new(
        "drogue",
        unsafe { &__storage as *const u8 as u32 },
        None,
        100,
        BluetoothMeshDriverConfig::default(),
    );

    // An instance of our device with the models we'd like to expose.
    let mut device = Device::new(board.blue_led, board.switch);

    // Run the mesh stack
    let _ = driver.run(&mut device).await;
}

// Application must run at a lower priority than softdevice. DO NOT CHANGE
fn config() -> Config {
    let mut config = Config::default();
    config.gpiote_interrupt_priority = interrupt::Priority::P2;
    config.time_interrupt_priority = interrupt::Priority::P2;
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
    pub fn new(led: RedLed, button: Switch) -> Self {
        Self {
            front: Front {
                led: MyOnOffServerHandler { led },
                button: MyOnOffClientHandler { button },
            },
        }
    }
}

struct MyOnOffServerHandler {
    led: RedLed,
}

impl BluetoothMeshModel<GenericOnOffServer> for MyOnOffServerHandler {
    async fn run<C: BluetoothMeshModelContext<GenericOnOffServer>>(
        &mut self,
        ctx: C,
    ) -> Result<(), ()> {
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

struct MyOnOffClientHandler {
    button: Switch,
}

impl BluetoothMeshModel<GenericOnOffClient> for MyOnOffClientHandler {
    #[allow(clippy::await_holding_refcell_ref)]
    async fn run<C: BluetoothMeshModelContext<GenericOnOffClient>>(
        &mut self,
        ctx: C,
    ) -> Result<(), ()> {
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

// Keeps our system alive
#[embassy_executor::task]
async fn watchdog_task() {
    let mut handle = unsafe { wdt::WatchdogHandle::steal(0) };
    loop {
        handle.pet();
        Timer::after(Duration::from_secs(2)).await;
    }
}
