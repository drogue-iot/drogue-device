#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::actors::dfu::{DfuCommand, FirmwareManager};
use drogue_device::ActorContext;
use embassy::executor::Spawner;
use embassy_boot_nrf::updater;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::{
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin, Pull},
    peripherals::P0_11,
    Peripherals,
};
use nrf_softdevice::{Flash, Softdevice};

#[cfg(feature = "panic-probe")]
use panic_probe as _;

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

#[cfg(feature = "panic-reset")]
use panic_reset as _;

// Application must run at a lower priority than softdevice
fn config() -> Config {
    let mut config = embassy_nrf::config::Config::default();
    config.gpiote_interrupt_priority = Priority::P2;
    config.time_interrupt_priority = Priority::P2;
    config
}

#[cfg(feature = "a")]
static FIRMWARE: &[u8] = include_bytes!("../b.bin");

#[embassy::main(config = "config()")]
async fn main(s: Spawner, p: Peripherals) {
    let sd = Softdevice::enable(&Default::default());
    s.spawn(softdevice_task(sd)).unwrap();

    let flash = Flash::take(sd);
    let updater = updater::new();

    static DFU: ActorContext<FirmwareManager<Flash>> = ActorContext::new();
    let dfu = DFU.mount(s, FirmwareManager::new(flash, updater));

    let button = Input::new(p.P0_11, Pull::Up);
    #[cfg(feature = "a")]
    let led = Output::new(p.P0_13.degrade(), Level::High, OutputDrive::Standard);

    #[cfg(feature = "b")]
    let led = Output::new(p.P0_16.degrade(), Level::High, OutputDrive::Standard);

    s.spawn(blinker(button, led)).unwrap();

    #[cfg(feature = "a")]
    {
        let mut dfu_button = Input::new(p.P0_12, Pull::Up);
        loop {
            dfu_button.wait_for_falling_edge().await;
            defmt::info!(
                "DFU process triggered. Reflashing with 'b' (size {} bytes)",
                FIRMWARE.len()
            );
            dfu.request(DfuCommand::Start).unwrap().await.unwrap();

            let mut offset = 0;
            for block in FIRMWARE.chunks(4096) {
                dfu.request(DfuCommand::WriteBlock(block))
                    .unwrap()
                    .await
                    .unwrap();
                offset += block.len();
            }

            dfu.request(DfuCommand::Finish).unwrap().await.unwrap();
        }
    }

    #[cfg(feature = "b")]
    {
        let mut dfu_button = Input::new(p.P0_12, Pull::Up);
        dfu_button.wait_for_falling_edge().await;
        dfu.request(DfuCommand::Booted).unwrap().await.unwrap();
    }
}

#[embassy::task]
async fn blinker(mut button: Input<'static, P0_11>, mut led: Output<'static, AnyPin>) {
    let mut high = false;
    loop {
        button.wait_for_falling_edge().await;

        if high {
            led.set_low();
            high = false;
        } else {
            led.set_high();
            high = true;
        }
    }
}

#[embassy::task]
async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}
