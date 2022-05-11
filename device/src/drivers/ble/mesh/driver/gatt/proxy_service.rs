#[nrf_softdevice::gatt_service(uuid = "00001828-0000-1000-8000-00805f9b34fb")]
pub struct ProxyService {
    #[characteristic(uuid = "00002add-0000-1000-8000-00805f9b34fb", write)]
    data_in: [u8; 33],
    #[characteristic(uuid = "00002ade-0000-1000-8000-00805f9b34fb", notify)]
    data_out: [u8; 33],
}
