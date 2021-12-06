use heapless::Vec;

#[nrf_softdevice::gatt_service(uuid = "0000180a-0000-1000-8000-00805f9b34fb")]
pub struct DeviceInformationService {
    #[characteristic(uuid = "00002a24-0000-1000-8000-00805f9b34fb", read)]
    pub model_number: Vec<u8, 32>,
    #[characteristic(uuid = "00002a25-0000-1000-8000-00805f9b34fb", read)]
    pub serial_number: Vec<u8, 32>,
    #[characteristic(uuid = "00002a27-0000-1000-8000-00805f9b34fb", read)]
    pub hardware_revision: Vec<u8, 4>,
    #[characteristic(uuid = "00002a29-0000-1000-8000-00805f9b34fb", read)]
    pub manufacturer_name: Vec<u8, 32>,
}
