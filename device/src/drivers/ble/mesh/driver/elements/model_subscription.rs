use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::model_subscription::ModelSubscriptionMessage;
use crate::drivers::ble::mesh::model::Status;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;

pub(crate) async fn dispatch<C: PrimaryElementContext>(
    ctx: &C,
    access: &AccessMessage,
    message: &ModelSubscriptionMessage,
) -> Result<(), DeviceError> {
    match message {
        ModelSubscriptionMessage::Add(add) => {
            let status = if ctx.is_local(&add.element_address) {
                let result = ctx
                    .update_configuration(|config| {
                        if let Some(network) = config.network_mut() {
                            network.subscriptions_mut().add(
                                add.element_address,
                                add.subscription_address,
                                add.model_identifier,
                            )?;
                            Ok(())
                        } else {
                            Err(DeviceError::NotProvisioned)
                        }
                    })
                    .await;

                match result {
                    Ok(_) => Status::Success,
                    Err(DeviceError::Status(status)) => status,
                    Err(all_others) => Err(all_others)?,
                }
            } else {
                Status::InvalidAddress
            };

            let response = add.create_status_response(status);
            ctx.transmit(access.create_response(ctx, ModelSubscriptionMessage::Status(response))?)
                .await?;
            Ok(())
        }
        //ModelSubscriptionMessage::VirtualAddressAdd(add) => {}
        //ModelSubscriptionMessage::Delete(_) => {}
        //ModelSubscriptionMessage::DeleteAll(_) => {}
        //ModelSubscriptionMessage::Overwrite(_) => {}
        //ModelSubscriptionMessage::VirtualAddressDelete(_) => {}
        //ModelSubscriptionMessage::VirtualAddressOverwrite(_) => {}
        _ => {
            // not applicable to server role
            Ok(())
        }
    }
}
