use crate::drivers::ble::mesh::driver::elements::NetworkDetails;
use crate::drivers::ble::mesh::driver::elements::PrimaryElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::foundation::configuration::{
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
            let response = if let Some(network) = ctx.network_details_by_app_key(bind.app_key_index)
            {
                let status = network
                    .model_app_bind(
                        bind.element_address,
                        bind.model_identifier,
                        bind.app_key_index,
                    )
                    .await?;

                ModelAppStatusMessage {
                    status,
                    payload: ModelAppPayload {
                        element_address: bind.element_address,
                        app_key_index: bind.app_key_index,
                        model_identifier: bind.model_identifier,
                    },
                }
            } else {
                ModelAppStatusMessage {
                    status: Status::InvalidAppKeyIndex,
                    payload: ModelAppPayload {
                        element_address: bind.element_address,
                        app_key_index: bind.app_key_index,
                        model_identifier: bind.model_identifier,
                    },
                }
            };

            ctx.transmit(access.create_response(ctx, ModelAppMessage::Status(response))?)
                .await?;
        }
        ModelAppMessage::Unbind(unbind) => {
            let response =
                if let Some(network) = ctx.network_details_by_app_key(unbind.app_key_index) {
                    let status = network
                        .model_app_unbind(
                            unbind.element_address,
                            unbind.model_identifier,
                            unbind.app_key_index,
                        )
                        .await?;

                    ModelAppStatusMessage {
                        status,
                        payload: ModelAppPayload {
                            element_address: unbind.element_address,
                            app_key_index: unbind.app_key_index,
                            model_identifier: unbind.model_identifier,
                        },
                    }
                } else {
                    ModelAppStatusMessage {
                        status: Status::InvalidAppKeyIndex,
                        payload: ModelAppPayload {
                            element_address: unbind.element_address,
                            app_key_index: unbind.app_key_index,
                            model_identifier: unbind.model_identifier,
                        },
                    }
                };

            ctx.transmit(access.create_response(ctx, ModelAppMessage::Status(response))?)
                .await?;
        }
        _ => {
            // not applicable to server role
        }
    }
    Ok(())
}
