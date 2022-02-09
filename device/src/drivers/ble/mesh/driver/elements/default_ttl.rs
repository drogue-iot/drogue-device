use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::DefaultTTLMessage;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;

pub(crate) async fn dispatch<C: PrimaryElementContext>(
    ctx: &C,
    access: &AccessMessage,
    message: &DefaultTTLMessage,
) -> Result<(), DeviceError> {
    match message {
        DefaultTTLMessage::Get => {
            let val = ctx.retrieve().configuration.default_ttl;
            ctx.transmit(access.create_response(ctx, DefaultTTLMessage::Status(val))?)
                .await?;
        }
        DefaultTTLMessage::Set(val) => {
            let mut update = ctx.retrieve();
            update.configuration.default_ttl = *val;
            ctx.store(update).await?;
            ctx.transmit(access.create_response(ctx, DefaultTTLMessage::Status(*val))?)
                .await?;
        }
        _ => {
            // not applicable to server role
        }
    }
    Ok(())
}
