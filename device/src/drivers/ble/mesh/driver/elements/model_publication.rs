use crate::drivers::ble::mesh::driver::elements::NetworkDetails;
use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::model_publication::ModelPublicationMessage;
use crate::drivers::ble::mesh::model::Status;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;

pub(crate) async fn dispatch<C: PrimaryElementContext>(
    ctx: &C,
    access: &AccessMessage,
    message: &ModelPublicationMessage,
) -> Result<(), DeviceError> {
    match message {
        ModelPublicationMessage::Set(set) => {
            let status = if let Some(network) = ctx.network_details_by_app_key(set.app_key_index) {
                network
                    .model_publication_set(
                        set.element_address,
                        set.publish_address.into(),
                        set.app_key_index,
                        set.credential_flag,
                        set.publish_ttl,
                        set.publish_period,
                        set.publish_retransmit_count,
                        set.publish_retransmit_interval_steps,
                        set.model_identifier,
                    )
                    .await?
            } else {
                Status::InvalidNetKeyIndex
            };
            ctx.transmit(access.create_response(
                ctx,
                ModelPublicationMessage::Status(set.create_status_response(status)),
            )?)
            .await?
        }
        //ModelPublicationMessage::Get(_) => {}
        //ModelPublicationMessage::VirtualAddressSet(_) => {}
        _ => {
            // not applicable to server role
        }
    }
    Ok(())
}
