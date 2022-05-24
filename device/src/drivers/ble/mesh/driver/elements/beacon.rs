use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::beacon::BeaconMessage;
use crate::drivers::ble::mesh::pdu::access::AccessMessage;

pub(crate) async fn dispatch<C: PrimaryElementContext>(
    ctx: &C,
    access: &AccessMessage,
    message: &BeaconMessage,
) -> Result<(), DeviceError> {
    match message {
        BeaconMessage::Get => {
            let val = ctx
                .configuration()
                .foundation_models()
                .configuration_model()
                .secure_beacon();
            ctx.transmit(access.create_response(ctx.address().ok_or(DeviceError::NotProvisioned)?, BeaconMessage::Status(val))?)
                .await?;
        }
        BeaconMessage::Set(val) => {
            ctx.update_configuration(|config| {
                *config
                    .foundation_models_mut()
                    .configuration_model_mut()
                    .secure_beacon_mut() = *val;
                Ok(())
            })
            .await?;
            ctx.transmit(access.create_response(ctx.address().ok_or(DeviceError::NotProvisioned)?, BeaconMessage::Status(*val))?)
                .await?;
        }
        _ => {
            // not applicable to server role
        }
    }
    Ok(())
}
