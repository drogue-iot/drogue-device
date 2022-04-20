# Bootloader for nRF with external flash capability

The bootloader uses `embassy-boot` to interact with the flash.

# Usage

Flash the bootloader

```
cargo flash --features embassy-nrf/nrf52832 --release --chip nRF52832_xxAA
```
