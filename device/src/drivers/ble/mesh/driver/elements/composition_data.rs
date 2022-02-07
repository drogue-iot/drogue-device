use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::{BeaconMessage, CompositionDataMessage, CompositionStatus};
use crate::drivers::ble::mesh::pdu::access::AccessMessage;

pub(crate) async fn dispatch<C: PrimaryElementContext>(
    ctx: &C,
    access: &AccessMessage,
    message: &CompositionDataMessage,
) -> Result<(), DeviceError> {
    match message {
        CompositionDataMessage::Get(page) => {
            if *page == 0 {
                ctx.transmit(access.create_response(ctx, CompositionDataMessage::Status(
                    CompositionStatus {
                        page: 0,
                        data: ctx.composition().clone(),
                    }
                ))?)
                    .await?;
            }
        }
        _ => {
            // not applicable to server role
        }
    }
    Ok(())
}
