use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::relay::RelayMessage;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;

pub(crate) async fn dispatch<C: PrimaryElementContext>(
    ctx: &C,
    access: &AccessMessage,
    message: &RelayMessage,
) -> Result<(), DeviceError> {
    match message {
        RelayMessage::Get => {
            let val = ctx
                .configuration()
                .foundation_models()
                .configuration_model()
                .relay()
                .clone();
            ctx.transmit(access.create_response(ctx.address().ok_or(DeviceError::NotProvisioned)?, RelayMessage::Status(val))?)
                .await?;
        }
        RelayMessage::Set(val) => {
            ctx.update_configuration(|config| {
                *config
                    .foundation_models_mut()
                    .configuration_model_mut()
                    .relay_mut() = *val;
                Ok(())
            })
            .await?;
            ctx.transmit(access.create_response(ctx.address().ok_or(DeviceError::NotProvisioned)?, RelayMessage::Status(*val))?)
                .await?;
        }
        _ => {
            // not applicable to server role
        }
    }
    Ok(())
}
