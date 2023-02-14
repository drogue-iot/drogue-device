#![no_std]
#![no_main]

#[cfg(feature = "defmt")]
use defmt_rtt as _;
use {
    cortex_m_rt::{entry, exception},
    embassy_boot_rp::*,
    embassy_rp::{
        flash::{Flash, ERASE_SIZE},
        peripherals::FLASH,
    },
};

const FLASH_SIZE: usize = 2 * 1024 * 1024;

#[entry]
fn main() -> ! {
    let p = embassy_rp::init(Default::default());

    // Uncomment this if you are debugging the bootloader with debugger/RTT attached,
    // as it prevents a hard fault when accessing flash 'too early' after boot.
    /*
    for i in 0..10000000 {
        cortex_m::asm::nop();
    }
    */

    let mut bl: BootLoader = BootLoader::default();
    let flash: Flash<'_, FLASH, FLASH_SIZE> = Flash::new(p.FLASH);
    let mut flash = BootFlash::<_, ERASE_SIZE>::new(flash);
    let start = bl.prepare(&mut SingleFlashConfig::new(&mut flash));
    core::mem::drop(flash);

    unsafe { bl.load(start) }
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