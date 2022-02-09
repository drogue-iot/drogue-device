use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::{AppKeyMessage, AppKeyStatusMessage};
use crate::drivers::ble::mesh::pdu::access::AccessMessage;
use crate::drivers::ble::mesh::driver::elements::NetworkDetails;

pub(crate) async fn dispatch<C: PrimaryElementContext>(
    ctx: &C,
    access: &AccessMessage,
    message: &AppKeyMessage,
) -> Result<(), DeviceError> {
    defmt::info!("PROCESS {}", message);
    match message {
        AppKeyMessage::Add(add) => {
            let status = ctx.network_details(add.indexes.net_key())
                .add_app_key(add.indexes.app_key(), add.app_key).await?;
            ctx.transmit(access.create_response(ctx, AppKeyMessage::Status(
                AppKeyStatusMessage {
                    status,
                    indexes: add.indexes
                }
            ))?).await?;
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
