#![no_std]
#![no_main]

use cortex_m_rt::{entry, exception};

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

use {
    adafruit_feather_nrf52::*, embassy_boot::FlashConfig, embassy_boot_nrf::*,
    embassy_nrf::nvmc::Nvmc,
};

#[entry]
fn main() -> ! {
    // Uncomment this if you are debugging the bootloader with debugger/RTT attached,
    // as it prevents a hard fault when accessing flash 'too early' after boot.
    /*
    for _i in 0..10000000 {
        cortex_m::asm::nop();
    }
    */

    let mut bl = BootLoader::default();

    let board = AdafruitFeatherNrf52::default();
    let qspi = board.external_flash.configure(interrupt::take!(QSPI));
    let nvmc = WatchdogFlash::start(Nvmc::new(board.nvmc), board.wdt, 5);
    let nvmc = BootFlash::new(nvmc);
    let qspi = BootFlash::new(qspi);
    let start = bl.prepare(&mut ExampleFlashConfig { nvmc, qspi });

    unsafe { bl.load(start) }
}

pub struct ExampleFlashConfig<'d> {
    nvmc: BootFlash<WatchdogFlash<'d>, 4096>,
    qspi: BootFlash<ExternalFlash<'d>, EXTERNAL_FLASH_BLOCK_SIZE>,
}

impl<'d> FlashConfig for ExampleFlashConfig<'d> {
    type STATE = BootFlash<WatchdogFlash<'d>, 4096>;
    type ACTIVE = BootFlash<WatchdogFlash<'d>, 4096>;
    type DFU = BootFlash<ExternalFlash<'d>, EXTERNAL_FLASH_BLOCK_SIZE>;

    fn active(&mut self) -> &mut Self::ACTIVE {
        &mut self.nvmc
    }

    fn state(&mut self) -> &mut Self::STATE {
        &mut self.nvmc
    }

    fn dfu(&mut self) -> &mut Self::DFU {
        &mut self.qspi
    }
}

#[no_mangle]
#[cfg_attr(target_os = "none", link_section = ".HardFault.user")]
unsafe extern "C" fn HardFault() {
    cortex_m::peripheral::SCB::sys_reset();
}

#[exception]
unsafe fn DefaultHandler(_: i16) -> ! {
    const SCB_ICSR: *const u32 = 0xE000_ED04 as *const u32;
    let irqn = core::ptr::read_volatile(SCB_ICSR) as u8 as i16 - 16;

    panic!("DefaultHandler #{:?}", irqn);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    cortex_m::asm::udf();
}
