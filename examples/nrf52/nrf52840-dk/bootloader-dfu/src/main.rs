#![no_std]
#![no_main]
#![macro_use]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use drogue_device::actors::dfu::{DfuCommand, FirmwareManager};
use drogue_device::actors::flash::{SharedFlash, SharedFlashHandle};
use drogue_device::ActorContext;
use embassy::executor::Spawner;
use embassy::time::{Duration, Timer};
use embassy_boot_nrf::updater;
use embassy_nrf::config::Config;
use embassy_nrf::interrupt::Priority;
use embassy_nrf::{
    gpio::{AnyPin, Input, Level, Output, OutputDrive, Pin, Pull},
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
    s.spawn(watchdog_task()).unwrap();

    let flash = Flash::take(sd);
    let updater = updater::new();

    static FLASH: ActorContext<SharedFlash<Flash>> = ActorContext::new();
    let flash = FLASH.mount(s, SharedFlash::new(flash));

    static DFU: ActorContext<FirmwareManager<SharedFlashHandle<Flash>>> = ActorContext::new();
    let dfu = DFU.mount(s, FirmwareManager::new(flash.into(), updater));

    #[cfg(feature = "a")]
    {
        let mut button = Input::new(p.P0_11, Pull::Up);
        //let mut button = Input::new(p.P1_02, Pull::Up);
        loop {
            button.wait_for_falling_edge().await;
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
        let led = Output::new(p.P0_13.degrade(), Level::High, OutputDrive::Standard);
        //let led = Output::new(p.P1_10.degrade(), Level::High, OutputDrive::Standard);
        s.spawn(blinker(led)).unwrap();

        //let mut button = Input::new(p.P0_11.degrade(), Pull::Up);
        let mut button = Input::new(p.P1_02, Pull::Up);
        button.wait_for_falling_edge().await;

        dfu.request(DfuCommand::Booted).unwrap().await.unwrap();
    }
}

#[embassy::task]
async fn blinker(mut led: Output<'static, AnyPin>) {
    loop {
        Timer::after(Duration::from_millis(300)).await;
        led.set_low();
        Timer::after(Duration::from_millis(300)).await;
        led.set_high();
    }
}

#[embassy::task]
async fn softdevice_task(sd: &'static Softdevice) {
    sd.run().await;
}

// Keeps our system alive
#[embassy::task]
async fn watchdog_task() {
    let mut handle = unsafe { embassy_nrf::wdt::WatchdogHandle::steal(0) };
    loop {
        handle.pet();
        Timer::after(Duration::from_secs(2)).await;
    }
}
