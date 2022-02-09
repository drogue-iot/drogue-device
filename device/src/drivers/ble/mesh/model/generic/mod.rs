use crate::drivers::ble::mesh::model::{
    Message, Model, ModelIdentifier,
};
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use crate::opcode;
use heapless::Vec;

pub struct GenericOnOffServer;

pub const GENERIC_ON_OFF_MODEL: ModelIdentifier = ModelIdentifier::SIG(0x1000);

pub enum GenericOnOffMessage {
    Get,
    Set(Set),
    SetUnacknowledged(Set),
    Status(Status),
}

impl Message for GenericOnOffMessage {
    fn opcode(&self) -> Opcode {
        match self {
            GenericOnOffMessage::Get => GENERIC_ON_OFF_GET,
            GenericOnOffMessage::Set(_) => GENERIC_ON_OFF_SET,
            GenericOnOffMessage::SetUnacknowledged(_) => GENERIC_ON_OFF_SET_UNACKNOWLEDGE,
            GenericOnOffMessage::Status(_) => GENERIC_ON_OFF_STATUS,
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut heapless::Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        match self {
            GenericOnOffMessage::Get => Ok(()),
            GenericOnOffMessage::Set(inner) => inner.emit_parameters(xmit),
            GenericOnOffMessage::SetUnacknowledged(inner) => inner.emit_parameters(xmit),
            GenericOnOffMessage::Status(inner) => inner.emit_parameters(xmit),
        }
    }
}

impl Model for GenericOnOffServer {
    const IDENTIFIER: ModelIdentifier = GENERIC_ON_OFF_MODEL;
    type MESSAGE = GenericOnOffMessage;

    fn parse(
        &self,
        opcode: Opcode,
        _parameters: &[u8],
    ) -> Result<Option<Self::MESSAGE>, ParseError> {
        match opcode {
            GENERIC_ON_OFF_GET => Ok(None),
            GENERIC_ON_OFF_SET => Ok(None),
            GENERIC_ON_OFF_SET_UNACKNOWLEDGE => Ok(None),
            _ => {
                // not applicable to this role
                Ok(None)
            }
        }
    }
}

opcode!( GENERIC_ON_OFF_GET 0x82, 0x01 );
opcode!( GENERIC_ON_OFF_SET 0x82, 0x02 );
opcode!( GENERIC_ON_OFF_SET_UNACKNOWLEDGE 0x82, 0x03 );
opcode!( GENERIC_ON_OFF_STATUS 0x82, 0x04 );

pub struct Set {
    on_off: u8,
    tid: u8,
    transition_time: u8,
    delay: u8,
}

impl Set {
    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.push(self.on_off).map_err(|_| InsufficientBuffer)?;
        xmit.push(self.tid).map_err(|_| InsufficientBuffer)?;
        xmit.push(self.transition_time)
            .map_err(|_| InsufficientBuffer)?;
        xmit.push(self.delay).map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}

pub struct Status {
    present_on_off: u8,
    target_on_off: u8,
    remaining_time: u8,
}

impl Status {
    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.push(self.present_on_off)
            .map_err(|_| InsufficientBuffer)?;
        xmit.push(self.target_on_off)
            .map_err(|_| InsufficientBuffer)?;
        xmit.push(self.remaining_time)
            .map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}
