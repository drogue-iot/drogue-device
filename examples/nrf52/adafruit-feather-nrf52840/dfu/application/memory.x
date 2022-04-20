MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  MBR                               : ORIGIN = 0x00000000, LENGTH = 4K
  SOFTDEVICE                        : ORIGIN = 0x00001000, LENGTH = 155648
  FLASH                             : ORIGIN = 0x00027000, LENGTH = 64K
  BOOTLOADER                        : ORIGIN = 0x000f9000, LENGTH = 24K
  BOOTLOADER_STATE                  : ORIGIN = 0x000ff000, LENGTH = 4K
  RAM                               : ORIGIN = 0x20020000, LENGTH = 128K

  /* DFU is in external flash */
  DFU                               : ORIGIN = 0x00000000, LENGTH = 68K
}

__bootloader_state_start = ORIGIN(BOOTLOADER_STATE);
__bootloader_state_end = ORIGIN(BOOTLOADER_STATE) + LENGTH(BOOTLOADER_STATE);

__bootloader_dfu_start = ORIGIN(DFU);
__bootloader_dfu_end = ORIGIN(DFU) + LENGTH(DFU);
