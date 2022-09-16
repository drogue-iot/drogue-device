#[nrf_softdevice::gatt_service(uuid = "b44fabf6-35b2-11ed-883f-d45d6455d2cc")]
pub struct ButtonsService {
    #[characteristic(uuid = "b4ad9022-35b2-11ed-a76a-d45d6455d2cc", read, notify)]
    pub presses: [u8; 2],
    #[characteristic(uuid = "b4f5ec00-35b2-11ed-a9a0-d45d6455d2cc", read, write)]
    pub period: u16,
}
