#![no_std]
#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]
pub(crate) mod fmt;

mod advertiser;
mod controller;
mod gatt;

pub use advertiser::*;
pub use controller::*;
pub use gatt::*;

//let temperature = TEMPERATURE.put(gatt_server::register(sd).unwrap());
/*

#[nrf_softdevice::gatt_server(uuid = "0000180a-0000-1000-8000-00805f9b34fb")]
struct DeviceInformationService {
    #[characteristic(uuid = "00002a24-0000-1000-8000-00805f9b34fb", read)]
    model_number: Vec<u8, 32>,
    #[characteristic(uuid = "00002a25-0000-1000-8000-00805f9b34fb", read)]
    serial_number: Vec<u8, 32>,
    #[characteristic(uuid = "00002a27-0000-1000-8000-00805f9b34fb", read)]
    hardware_revision: Vec<u8, 4>,
    #[characteristic(uuid = "00002a29-0000-1000-8000-00805f9b34fb", read)]
    manufacturer_name: Vec<u8, 32>,
}
*/
