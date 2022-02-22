use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::app_key::{
    AppKeyListMessage, AppKeyMessage, AppKeyStatusMessage,
};
use crate::drivers::ble::mesh::model::Status;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;

pub(crate) async fn dispatch<C: PrimaryElementContext>(
    ctx: &C,
    access: &AccessMessage,
    message: &AppKeyMessage,
) -> Result<(), DeviceError> {
    match message {
        AppKeyMessage::Add(add) => {
            let result = ctx
                .update_configuration(|config| {
                    if let Some(network) = config.network_mut() {
                        if let Ok(network) =
                            network.find_by_net_key_index_mut(&add.indexes.net_key())
                        {
                            network.add_app_key(add.indexes.app_key(), add.app_key)?;
                            Ok(())
                        } else {
                            Err(Status::InvalidNetKeyIndex)?
                        }
                    } else {
                        Err(DeviceError::NotProvisioned)
                    }
                })
                .await;

            let status = match result {
                Ok(_) => Status::Success,
                Err(DeviceError::Status(status)) => status,
                Err(all_others) => Err(all_others)?,
            };

            let response = AppKeyStatusMessage {
                status,
                indexes: add.indexes,
            };

            ctx.transmit(access.create_response(ctx, AppKeyMessage::Status(response))?)
                .await?;
        }
        AppKeyMessage::Get(get) => {
            let result = if let Some(networks) = ctx.configuration().network() {
                if let Ok(network) = networks.find_by_net_key_index(&get.net_key_index) {
                    Ok(network.app_keys_iter().map(|e| e.index).collect())
                } else {
                    Err(Status::InvalidNetKeyIndex)?
                }
            } else {
                Err(Status::UnspecifiedError)?
            };

            let response = match result {
                Ok(indexes) => AppKeyListMessage {
                    status: Status::Success,
                    net_key_index: get.net_key_index,
                    app_key_indexes: indexes,
                },
                Err(status) => AppKeyListMessage {
                    status,
                    net_key_index: get.net_key_index,
                    app_key_indexes: Default::default(),
                },
            };

            ctx.transmit(access.create_response(ctx, AppKeyMessage::List(response))?)
                .await?;
        }
        /*
        AppKeyMessage::Delete(delete) => {
            ctx.network_details(delete.net_key_index)
                .delete_app_key(delete.app_key_index).await;
        }
        AppKeyMessage::Get(get) => {
            ctx.network_details(get.net_key_index)
                .get_app_key(get.app_key_index);
        }
        AppKeyMessage::List(list) => {
            let keys = ctx.network_details(delete.net_key_index)
                .app_keys(delete.app_key_index);

        }
        AppKeyMessage::Update(update) => {
            let result = ctx.network_details(delete.net_key_index)
                .update_app_key(update.app_key_index, update.app_key).await;
        }
         */
        _ => {
            // not applicable to server role
        }
    }
    Ok(())
}
