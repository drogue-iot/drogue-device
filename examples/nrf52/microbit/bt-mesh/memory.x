MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  MBR                               : ORIGIN = 0x00000000, LENGTH = 4K
  SOFTDEVICE                        : ORIGIN = 0x00001000, LENGTH = 155648
  FLASH                             : ORIGIN = 0x00027000, LENGTH = 139264
  DFU                               : ORIGIN = 0x00049000, LENGTH = 143360
  BOOTLOADER                        : ORIGIN = 0x0006c000, LENGTH = 24K
  BOOTLOADER_STATE                  : ORIGIN = 0x00072000, LENGTH = 4K
  STORAGE                           : ORIGIN = 0x00073000, LENGTH = 4K
  RAM                               : ORIGIN = 0x20002988, LENGTH = 120440
}

__bootloader_state_start = ORIGIN(BOOTLOADER_STATE);
__bootloader_state_end = ORIGIN(BOOTLOADER_STATE) + LENGTH(BOOTLOADER_STATE);

/*
__bootloader_dfu_start = ORIGIN(DFU);
__bootloader_dfu_end = ORIGIN(DFU) + LENGTH(DFU);
*/

__storage = ORIGIN(STORAGE);
