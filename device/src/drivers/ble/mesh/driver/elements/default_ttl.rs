use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::default_ttl::DefaultTTLMessage;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;

pub(crate) async fn dispatch<C: PrimaryElementContext>(
    ctx: &C,
    access: &AccessMessage,
    message: &DefaultTTLMessage,
) -> Result<(), DeviceError> {
    match message {
        DefaultTTLMessage::Get => {
            let val = ctx
                .configuration()
                .foundation_models()
                .configuration_model()
                .default_ttl();
            ctx.transmit(access.create_response(
                ctx.address().ok_or(DeviceError::NotProvisioned)?,
                DefaultTTLMessage::Status(val),
            )?)
            .await?;
        }
        DefaultTTLMessage::Set(val) => {
            ctx.update_configuration(|config| {
                *config
                    .foundation_models_mut()
                    .configuration_model_mut()
                    .default_ttl_mut() = *val;
                Ok(())
            })
            .await?;
            ctx.transmit(access.create_response(
                ctx.address().ok_or(DeviceError::NotProvisioned)?,
                DefaultTTLMessage::Status(*val),
            )?)
            .await?;
        }
        _ => {
            // not applicable to server role
        }
    }
    Ok(())
}
