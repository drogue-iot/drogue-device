use crate::drivers::ble::mesh::device::Uuid;
use crate::drivers::ble::mesh::driver::node::{NetworkId, State};
use crate::drivers::ble::mesh::interface::{Beacon, BearerError, GattBearer, NetworkError, PDU};
use crate::drivers::ble::mesh::pdu::network::ObfuscatedAndEncryptedNetworkPDU;
use crate::drivers::ble::mesh::pdu::proxy::{MessageType, ProxyPDU, SAR};
use crate::drivers::ble::mesh::provisioning::ProvisioningPDU;
use core::cell::Cell;
use heapless::Vec;

pub struct GattBearerNetworkInterface<B: GattBearer<MTU>, const MTU: usize> {
    uuid: Cell<Option<Uuid>>,
    bearer: B,
}

impl<B: GattBearer<MTU>, const MTU: usize> GattBearerNetworkInterface<B, MTU> {
    pub fn new(bearer: B) -> Self {
        Self {
            uuid: Cell::new(None),
            bearer,
        }
    }

    pub(super) fn set_network_id(&self, network_id: NetworkId) {
        self.bearer.set_network_id(network_id);
    }

    pub(super) fn set_uuid(&self, uuid: Uuid) {
        self.uuid.replace(Some(uuid));
    }

    pub(super) fn set_state(&self, state: State) {
        self.bearer.set_state(state);
    }

    pub async fn run(&self) -> Result<(), NetworkError> {
        self.bearer.run().await?;
        Ok(())
    }

    pub async fn receive(&self) -> Result<PDU, BearerError> {
        loop {
            let data = self.bearer.receive().await?;
            let proxy_pdu = ProxyPDU::parse(&data)?;
            if let SAR::Complete = proxy_pdu.sar {
                match proxy_pdu.message_type {
                    MessageType::NetworkPDU => {
                        let pdu = ObfuscatedAndEncryptedNetworkPDU::parse(&proxy_pdu.data)?;
                        return Ok(PDU::Network(pdu));
                    }
                    MessageType::MeshBeacon => {}
                    MessageType::ProxyConfiguration => {}
                    MessageType::ProvisioningPDU => {
                        let pdu = ProvisioningPDU::parse(&proxy_pdu.data)?;
                        return Ok(PDU::Provisioning(pdu));
                    }
                }
            }
        }
    }

    pub async fn transmit(&self, pdu: &PDU) -> Result<(), BearerError> {
        match pdu {
            PDU::Provisioning(pdu) => {
                let mut all_proxy_data = Vec::<u8, 384>::new();
                pdu.emit(&mut all_proxy_data)?;
                let mut data = Vec::new();
                data.extend_from_slice(&all_proxy_data)?;
                let proxy_pdu = ProxyPDU {
                    sar: SAR::Complete,
                    message_type: MessageType::ProvisioningPDU,
                    data,
                };

                self.transmit_proxy_pdu(&proxy_pdu).await
            }
            PDU::Network(pdu) => {
                let mut all_proxy_data = Vec::<u8, 384>::new();
                pdu.emit(&mut all_proxy_data)?;
                let mut data = Vec::new();
                data.extend_from_slice(&all_proxy_data)?;
                let proxy_pdu = ProxyPDU {
                    sar: SAR::Complete,
                    message_type: MessageType::NetworkPDU,
                    data,
                };

                self.transmit_proxy_pdu(&proxy_pdu).await
            }
        }
    }

    async fn transmit_proxy_pdu(&self, pdu: &ProxyPDU) -> Result<(), BearerError> {
        let mut bytes = Vec::new();
        pdu.emit(&mut bytes)?;
        self.bearer.transmit(&bytes).await
    }

    pub async fn beacon(&self, beacon: Beacon) -> Result<(), BearerError> {
        match beacon {
            Beacon::Unprovisioned => {
                if let Some(uuid) = self.uuid.get() {
                    let mut adv_data = Vec::new();

                    #[rustfmt::skip]
                    adv_data
                        .extend_from_slice(&[
                            0x02, 0x01, 0x06,
                            0x03, 0x03, 0x27, 0x18,
                            0x15, 0x16, 0x27, 0x18
                        ]).unwrap();

                    adv_data.extend_from_slice(&uuid.0).unwrap();

                    // TODO fix OOB data values
                    adv_data.extend_from_slice(&[0x00, 0x00]).unwrap();

                    self.bearer.advertise(&adv_data).await?;
                }
            }
            Beacon::Provisioned(network_id) => {
                let mut adv_data = Vec::new();

                #[rustfmt::skip]
                adv_data.extend_from_slice(&[
                    0x02, 0x01, 0x06,
                    0x03, 0x03, 0x28, 0x18,
                    0x0C, 0x16, 0x28, 0x18
                ]).unwrap();

                adv_data.push(0x00)?; // network id
                adv_data.extend_from_slice(&network_id.0)?;
                self.bearer.advertise(&adv_data).await?;
            }
            Beacon::Secure => {
                // nothing yet
            }
        }

        Ok(())
    }
}
