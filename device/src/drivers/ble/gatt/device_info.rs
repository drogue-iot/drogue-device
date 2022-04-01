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

impl DeviceInformationService {
    pub fn initialize(
        &self,
        model: &[u8],
        serial_number: &[u8],
        manufacturer_name: &[u8],
        hardware_revision: &[u8],
    ) -> Result<(), ()> {
        self.model_number_set(Vec::from_slice(model)?)
            .map_err(|_| ())?;
        self.serial_number_set(Vec::from_slice(serial_number)?)
            .map_err(|_| ())?;
        self.manufacturer_name_set(Vec::from_slice(manufacturer_name)?)
            .map_err(|_| ())?;
        self.hardware_revision_set(Vec::from_slice(hardware_revision)?)
            .map_err(|_| ())?;
        Ok(())
    }
}
