use crate::drivers::ble::mesh::driver::pipeline::mesh::MeshContext;
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::segmentation::outbound::{
    OutboundSegments, OutboundSegmentsIter,
};
use crate::drivers::ble::mesh::driver::pipeline::unprovisioned::segmentation::Segmentation;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::generic_provisioning::{
    GenericProvisioningPDU, ProvisioningBearerControl, Reason,
};
use crate::drivers::ble::mesh::pdu::bearer::advertising::AdvertisingPDU;
use crate::drivers::ble::mesh::provisioning::ProvisioningPDU;
use core::iter::Iterator;
use defmt::Format;

pub struct ProvisioningBearer {
    segmentation: Segmentation,
    link_id: Option<u32>,
    inbound_transaction_number: Option<u8>,
    acked_inbound_transaction_number: Option<u8>,
    outbound_pdu: Option<OutboundPDU>,
    outbound_transaction_number: u8,
}

impl Default for ProvisioningBearer {
    fn default() -> Self {
        Self {
            segmentation: Segmentation::default(),
            link_id: None,
            inbound_transaction_number: None,
            acked_inbound_transaction_number: None,
            outbound_pdu: None,
            outbound_transaction_number: 0x80,
        }
    }
}

#[derive(Format)]
pub enum BearerMessage {
    ProvisioningPDU(ProvisioningPDU),
    Close(Reason),
}

impl ProvisioningBearer {
    pub fn reset(&mut self) {
        self.link_id.take();
        self.inbound_transaction_number.take();
        self.acked_inbound_transaction_number.take();
        self.outbound_pdu.take();
        self.outbound_transaction_number = 0x80;
    }

    pub async fn process_inbound<C: MeshContext>(
        &mut self,
        ctx: &C,
        pdu: AdvertisingPDU,
    ) -> Result<Option<BearerMessage>, DeviceError> {
        match pdu.pdu {
            GenericProvisioningPDU::ProvisioningBearerControl(pbc) => {
                match pbc {
                    ProvisioningBearerControl::LinkOpen(uuid) => {
                        if ctx.uuid() == uuid {
                            if let None = self.link_id {
                                self.inbound_transaction_number
                                    .replace(pdu.transaction_number);
                                self.link_id.replace(pdu.link_id);

                                ctx.transmit_advertising_pdu(AdvertisingPDU {
                                    link_id: pdu.link_id,
                                    transaction_number: 0,
                                    pdu: GenericProvisioningPDU::ProvisioningBearerControl(
                                        ProvisioningBearerControl::LinkAck,
                                    ),
                                })
                                .await?;
                                Ok(None)
                            } else if let Some(link_id) = self.link_id {
                                if link_id == pdu.link_id {
                                    // just keep LinkAck'ing it.
                                    ctx.transmit_advertising_pdu(AdvertisingPDU {
                                        link_id: pdu.link_id,
                                        transaction_number: 0,
                                        pdu: GenericProvisioningPDU::ProvisioningBearerControl(
                                            ProvisioningBearerControl::LinkAck,
                                        ),
                                    })
                                    .await?;
                                    Ok(None)
                                } else {
                                    Err(DeviceError::InvalidLink)
                                }
                            } else {
                                Err(DeviceError::InvalidLink)
                            }
                        } else {
                            Ok(None)
                        }
                    }
                    ProvisioningBearerControl::LinkAck => {
                        /* not applicable for this role */
                        Ok(None)
                    }
                    ProvisioningBearerControl::LinkClose(reason) => {
                        self.link_id.take();
                        self.inbound_transaction_number.take();
                        Ok(Some(BearerMessage::Close(reason)))
                    }
                }
            }
            GenericProvisioningPDU::TransactionStart(_)
            | GenericProvisioningPDU::TransactionContinuation(_) => {
                if self.should_process_transaction(pdu.transaction_number) {
                    let result = self.segmentation.process_inbound(pdu.pdu).await;
                    if let Ok(Some(result)) = result {
                        self.ack_transaction(ctx).await?;
                        Ok(Some(BearerMessage::ProvisioningPDU(result)))
                    } else {
                        Ok(None)
                    }
                } else {
                    self.try_ack_transaction_again(ctx, pdu.transaction_number)
                        .await?;
                    self.try_retransmit(ctx).await?;
                    Ok(None)
                }
            }
            GenericProvisioningPDU::TransactionAck => {
                if let Some(outbound) = &self.outbound_pdu {
                    if outbound.transaction_number == pdu.transaction_number {
                        // They heard us, we can stop retransmitting.
                        self.outbound_pdu.take();
                    }
                }
                Ok(None)
            }
        }
    }

    fn should_process_transaction(&mut self, transaction_number: u8) -> bool {
        match (
            self.inbound_transaction_number,
            self.acked_inbound_transaction_number,
        ) {
            (Some(inbound), _) if inbound == transaction_number => {
                // This transaction is still being collected
                true
            }
            (None, Some(acked)) if acked < transaction_number => {
                // No current transaction, let's go.
                self.inbound_transaction_number.replace(transaction_number);
                true
            }
            _ => {
                // Either current transaction is different or it's already
                // been acked.
                false
            }
        }
    }

    async fn try_ack_transaction_again<C: MeshContext>(
        &mut self,
        ctx: &C,
        transaction_number: u8,
    ) -> Result<(), DeviceError> {
        if let Some(acked) = self.acked_inbound_transaction_number {
            if acked >= transaction_number {
                ctx.transmit_advertising_pdu(AdvertisingPDU {
                    link_id: self.link_id.ok_or(DeviceError::InvalidLink)?,
                    transaction_number,
                    pdu: GenericProvisioningPDU::TransactionAck,
                })
                .await?;
            }
        }
        Ok(())
    }

    async fn ack_transaction<C: MeshContext>(&mut self, ctx: &C) -> Result<bool, DeviceError> {
        match (
            self.inbound_transaction_number,
            self.acked_inbound_transaction_number,
        ) {
            // TODO dry up this repetition
            (Some(current), Some(last_ack)) if current > last_ack => {
                ctx.transmit_advertising_pdu(AdvertisingPDU {
                    link_id: self.link_id.ok_or(DeviceError::InvalidLink)?,
                    transaction_number: current,
                    pdu: GenericProvisioningPDU::TransactionAck,
                })
                .await?;
                self.acked_inbound_transaction_number.replace(current);
                self.inbound_transaction_number.take();
                Ok(true)
            }
            (Some(current), None) => {
                ctx.transmit_advertising_pdu(AdvertisingPDU {
                    link_id: self.link_id.ok_or(DeviceError::InvalidLink)?,
                    transaction_number: current,
                    pdu: GenericProvisioningPDU::TransactionAck,
                })
                .await?;
                self.acked_inbound_transaction_number.replace(current);
                self.inbound_transaction_number.take();
                Ok(true)
            }
            _ => Err(DeviceError::InvalidTransactionNumber),
        }
    }

    pub async fn try_retransmit<C: MeshContext>(&mut self, ctx: &C) -> Result<(), DeviceError> {
        if let Some(outbound) = &self.outbound_pdu {
            for pdu in outbound.iter() {
                ctx.transmit_advertising_pdu(pdu).await?
            }
        }
        Ok(())
    }

    pub async fn process_outbound(
        &mut self,
        pdu: ProvisioningPDU,
    ) -> Result<impl Iterator<Item = AdvertisingPDU> + '_, DeviceError> {
        let segments = self.segmentation.process_outbound(pdu).await?;

        let transaction_number = self.outbound_transaction_number;
        self.outbound_transaction_number = self.outbound_transaction_number + 1;

        self.outbound_pdu.replace(OutboundPDU {
            link_id: self.link_id.ok_or(DeviceError::InvalidLink)?,
            transaction_number,
            segments: segments,
        });

        Ok(self.outbound_pdu.as_mut().unwrap().iter())
    }
}

pub struct OutboundPDU {
    link_id: u32,
    transaction_number: u8,
    segments: OutboundSegments,
}

impl OutboundPDU {
    pub fn iter(&self) -> OutboundPDUIter {
        OutboundPDUIter {
            link_id: self.link_id,
            transaction_number: self.transaction_number,
            inner: self.segments.iter(),
        }
    }
}

pub struct OutboundPDUIter<'i> {
    link_id: u32,
    transaction_number: u8,
    inner: OutboundSegmentsIter<'i>,
}

impl<'i> OutboundPDUIter<'i> {
    fn new(inner: OutboundSegmentsIter<'i>, link_id: u32, transaction_number: u8) -> Self {
        Self {
            link_id,
            transaction_number,
            inner,
        }
    }
}

impl<'i> Iterator for OutboundPDUIter<'i> {
    type Item = AdvertisingPDU;

    fn next(&mut self) -> Option<Self::Item> {
        let inner = self.inner.next();
        match inner {
            None => None,
            Some(pdu) => Some(AdvertisingPDU {
                link_id: self.link_id,
                transaction_number: self.transaction_number,
                pdu,
            }),
        }
    }
}
