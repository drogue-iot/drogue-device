use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::model::foundation::configuration::BeaconMessage;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;

pub(crate) async fn dispatch<C:PrimaryElementContext>(ctx: &C, access: &AccessMessage, message: &BeaconMessage) -> Result<(), DeviceError>{
    match message {
        BeaconMessage::Get => {
            let val = ctx.retrieve().configuration.secure_beacon;
            ctx.transmit(access.create_response(ctx, BeaconMessage::Status(val))?)
                .await?;
        }
        BeaconMessage::Set(val) => {
            let mut update = ctx.retrieve();
            update.configuration.secure_beacon = *val;
            ctx.store(update).await?;
            ctx.transmit(access.create_response(ctx, BeaconMessage::Status(*val))?)
                .await?;
        }
        _ => {
            // not applicable to server role
        }
    }
    Ok(())
}