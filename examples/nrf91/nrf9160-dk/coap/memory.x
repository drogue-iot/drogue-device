MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  /* Assumes Secure Partition Manager (SPM) flashed at the start */
  FLASH                             : ORIGIN = 0x00050000, LENGTH = 512K
  RAM                         (rwx) : ORIGIN = 0x20018000, LENGTH = 160K
}
