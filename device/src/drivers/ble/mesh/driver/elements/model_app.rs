use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::model_app::{
    ModelAppMessage, ModelAppPayload, ModelAppStatusMessage,
};
use crate::drivers::ble::mesh::model::Status;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;

pub(crate) async fn dispatch<C: PrimaryElementContext>(
    ctx: &C,
    access: &AccessMessage,
    message: &ModelAppMessage,
) -> Result<(), DeviceError> {
    match message {
        ModelAppMessage::Bind(bind) => {
            let result = ctx
                .update_configuration(|config| {
                    if let Some(network) = config.network_mut() {
                        if let Ok(network) = network.find_by_app_key_index_mut(&bind.app_key_index)
                        {
                            network.bind(
                                &bind.element_address,
                                &bind.model_identifier,
                                &bind.app_key_index,
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

            let response = ModelAppStatusMessage {
                status,
                payload: ModelAppPayload {
                    element_address: bind.element_address,
                    app_key_index: bind.app_key_index,
                    model_identifier: bind.model_identifier,
                },
            };

            ctx.transmit(access.create_response(ctx.address().ok_or(DeviceError::NotProvisioned)?, ModelAppMessage::Status(response))?)
                .await?;
        }
        ModelAppMessage::Unbind(unbind) => {
            let result = ctx
                .update_configuration(|config| {
                    if let Some(network) = config.network_mut() {
                        if let Ok(network) =
                            network.find_by_app_key_index_mut(&unbind.app_key_index)
                        {
                            network.unbind(&unbind.element_address, &unbind.model_identifier)?;
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

            let response = ModelAppStatusMessage {
                status,
                payload: ModelAppPayload {
                    element_address: unbind.element_address,
                    app_key_index: unbind.app_key_index,
                    model_identifier: unbind.model_identifier,
                },
            };

            ctx.transmit(access.create_response(ctx.address().ok_or(DeviceError::NotProvisioned)?, ModelAppMessage::Status(response))?)
                .await?;
        }
        _ => {
            // not applicable to server role
        }
    }
    Ok(())
}
