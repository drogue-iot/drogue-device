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
            let result = ctx
                .update_configuration(|config| {
                    info!("pub A");
                    if let Some(network) = config.network_mut() {
                        info!("pub B");
                        if let Ok(network) = network.find_by_app_key_index_mut(&set.app_key_index) {
                            info!("pub C");
                            network.publications_mut().set(
                                set.element_address,
                                set.publish_address.into(),
                                set.app_key_index,
                                set.credential_flag,
                                set.publish_ttl,
                                set.publish_period,
                                set.publish_retransmit_count,
                                set.publish_retransmit_interval_steps,
                                set.model_identifier,
                            )?;
                            Ok(())
                        } else {
                            Err(Status::InvalidAppKeyIndex)?
                        }
                    } else {
                        Err(DeviceError::NotProvisioned)?
                    }
                })
                .await;

            let status = match result {
                Ok(_) => Status::Success,
                Err(DeviceError::Status(status)) => status,
                Err(all_others) => Err(all_others)?,
            };

            ctx.transmit(access.create_response(
                ctx,
                ModelPublicationMessage::Status(set.create_status_response(status)),
            )?)
            .await?;
        }
        //ModelPublicationMessage::Get(_) => {}
        //ModelPublicationMessage::VirtualAddressSet(_) => {}
        _ => {
            // not applicable to server role
        }
    }
    Ok(())
}
