#[nrf_softdevice::gatt_service(uuid = "e95d6100-251d-470a-a062-fa1922dfa9a8")]
pub struct TemperatureService {
    #[characteristic(uuid = "e95d9250-251d-470a-a062-fa1922dfa9a8", read, notify)]
    pub temperature: i8,
    #[characteristic(uuid = "e95d1b25-251d-470a-a062-fa1922dfa9a8", read, write)]
    pub period: u16,
}
