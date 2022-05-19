MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  MBR                               : ORIGIN = 0x00000000, LENGTH = 4K
  SOFTDEVICE                        : ORIGIN = 0x00001000, LENGTH = 114688
  FLASH                             : ORIGIN = 0x00027000, LENGTH = 159744
  DFU                               : ORIGIN = 0x0004E000, LENGTH = 163840
  BOOTLOADER                        : ORIGIN = 0x00077000, LENGTH = 24K
  BOOTLOADER_STATE                  : ORIGIN = 0x0007D000, LENGTH = 4K
  STORAGE                           : ORIGIN = 0x0007F000, LENGTH = 4K
  RAM                               : ORIGIN = 0x2000CD28, LENGTH = 78552
}

__bootloader_state_start = ORIGIN(BOOTLOADER_STATE);
__bootloader_state_end = ORIGIN(BOOTLOADER_STATE) + LENGTH(BOOTLOADER_STATE);

__bootloader_dfu_start = ORIGIN(DFU);
__bootloader_dfu_end = ORIGIN(DFU) + LENGTH(DFU);

__storage = ORIGIN(STORAGE);
