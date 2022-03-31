MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  MBR                               : ORIGIN = 0x00000000, LENGTH = 4K
  SOFTDEVICE                        : ORIGIN = 0x00001000, LENGTH = 114688
  FLASH                             : ORIGIN = 0x0001C000, LENGTH = 188416
  DFU                               : ORIGIN = 0x0004a000, LENGTH = 192512
  BOOTLOADER                        : ORIGIN = 0x00079000, LENGTH = 24K
  BOOTLOADER_STATE                  : ORIGIN = 0x0007f000, LENGTH = 4K
  RAM                               : ORIGIN = 0x2000baa8, LENGTH = 83288
}

__bootloader_state_start = ORIGIN(BOOTLOADER_STATE);
__bootloader_state_end = ORIGIN(BOOTLOADER_STATE) + LENGTH(BOOTLOADER_STATE);

__bootloader_dfu_start = ORIGIN(DFU);
__bootloader_dfu_end = ORIGIN(DFU) + LENGTH(DFU);
