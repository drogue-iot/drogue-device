use crate::drivers::ble::mesh::driver::gatt::proxy_service::*;

pub mod proxy_service;

#[nrf_softdevice::gatt_server]
pub struct ProxyServer {
    proxy: ProxyService,
}
