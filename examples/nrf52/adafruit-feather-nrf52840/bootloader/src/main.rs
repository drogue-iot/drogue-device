#![no_std]
#![no_main]

use cortex_m_rt::{entry, exception};

#[cfg(feature = "defmt-rtt")]
use defmt_rtt as _;

use drogue_device::{boards::nrf52::adafruit_feather_nrf52840::*, Board};
use embassy_boot::FlashConfig;
use embassy_boot_nrf::*;
use embassy_nrf::nvmc::Nvmc;

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
    let p = embassy_nrf::init(Default::default());
    let board = AdafruitFeatherNrf52840::new(p);

    let start = {
        let start = bl.prepare(&mut ExampleFlashConfig {
            nvmc: &mut BootFlash::new(&mut WatchdogFlash::start(Nvmc::new(board.nvmc), board.wdt, 5)),
            qspi: &mut BootFlash::new(&mut board.external_flash.configure()),
        });
        start
    };

    unsafe { bl.load(start) }
}

pub struct ExampleFlashConfig<'d> {
    nvmc: &'d mut BootFlash<'d, WatchdogFlash<'d>, 4096>,
    qspi: &'d mut BootFlash<'d, ExternalFlash<'d>, EXTERNAL_FLASH_BLOCK_SIZE>,
}

impl<'d> FlashConfig for ExampleFlashConfig<'d> {
    type STATE = BootFlash<'d, WatchdogFlash<'d>, 4096>;
    type ACTIVE = BootFlash<'d, WatchdogFlash<'d>, 4096>;
    type DFU = BootFlash<'d, ExternalFlash<'d>, EXTERNAL_FLASH_BLOCK_SIZE>;

    fn active(&mut self) -> &mut Self::ACTIVE {
        self.nvmc
    }

    fn state(&mut self) -> &mut Self::STATE {
        self.nvmc
    }

    fn dfu(&mut self) -> &mut Self::DFU {
        self.qspi
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
