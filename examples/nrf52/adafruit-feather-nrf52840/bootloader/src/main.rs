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
    let p = embassy_nrf::init(Default::default());

    // Uncomment this if you are debugging the bootloader with debugger/RTT attached,
    // as it prevents a hard fault when accessing flash 'too early' after boot.
    /*
    for _i in 0..10000000 {
        cortex_m::asm::nop();
    }
    */

    let board = AdafruitFeatherNrf52840::new(p);
    let mut bl = BootLoader::default();
    let q = board.external_flash.configure();
    let mut provider = ExampleFlashProvider {
        nvmc: NvmcFlashConfig {
            nvmc: WatchdogFlash::start(Nvmc::new(board.nvmc), board.wdt, 5),
        },
        qspi: QspiFlashConfig { qspi: q },
    };

    let start = bl.prepare(&mut provider);
    core::mem::drop(provider);

    unsafe { bl.load(start) }
}

pub struct ExampleFlashProvider<'d> {
    nvmc: NvmcFlashConfig<'d>,
    qspi: QspiFlashConfig<'d>,
}

pub struct NvmcFlashConfig<'d> {
    nvmc: WatchdogFlash<'d>,
}

impl<'d> FlashConfig for NvmcFlashConfig<'d> {
    type FLASH = WatchdogFlash<'d>;
    const BLOCK_SIZE: usize = 4096;

    fn flash(&mut self) -> &mut Self::FLASH {
        &mut self.nvmc
    }
}

pub struct QspiFlashConfig<'d> {
    qspi: ExternalFlash<'d>,
}

impl<'d> FlashConfig for QspiFlashConfig<'d> {
    type FLASH = ExternalFlash<'d>;
    const BLOCK_SIZE: usize = EXTERNAL_FLASH_BLOCK_SIZE;

    fn flash(&mut self) -> &mut Self::FLASH {
        &mut self.qspi
    }
}

impl<'d> FlashProvider for ExampleFlashProvider<'d> {
    type STATE = NvmcFlashConfig<'d>;
    type ACTIVE = NvmcFlashConfig<'d>;
    type DFU = QspiFlashConfig<'d>;

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
