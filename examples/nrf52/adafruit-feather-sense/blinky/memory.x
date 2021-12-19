MEMORY
{
  /* These values correspond to the NRF52840 with Softdevices S140 7.0.1 */
  FLASH : ORIGIN = 0x00026000, LENGTH = 0xED000 - 0x26000
  RAM : ORIGIN = 0x20003400, LENGTH = 256K - 0x3400
}
