MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  FLASH                             : ORIGIN = 0x00027000, LENGTH = 356352
  CONFIG                            : ORIGIN = 0x0007E000, LENGTH = 4K
  STORAGE                           : ORIGIN = 0x0007F000, LENGTH = 4K
  RAM                               : ORIGIN = 0x20002988, LENGTH = 120440
}

__config = ORIGIN(CONFIG);
__storage = ORIGIN(STORAGE);
