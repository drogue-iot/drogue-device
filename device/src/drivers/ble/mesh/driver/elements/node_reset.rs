use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::node_reset::NodeResetMessage;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;

pub(crate) async fn dispatch<C: PrimaryElementContext>(
    ctx: &C,
    access: &AccessMessage,
    message: &NodeResetMessage,
) -> Result<(), DeviceError> {
    match message {
        NodeResetMessage::Reset => {
            ctx.transmit(access.create_response(ctx, NodeResetMessage::Status)?)
                .await?;
            ctx.node_reset().await;
        }
        _ => {
            // not applicable to server role
        }
    }
    Ok(())
}
