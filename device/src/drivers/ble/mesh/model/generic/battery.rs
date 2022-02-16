use crate::drivers::ble::mesh::model::{Message, Model, ModelIdentifier};
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use crate::opcode;
use heapless::Vec;

pub struct GenericBatteryServer;

pub const GENERIC_BATTERY_SERVER: ModelIdentifier = ModelIdentifier::SIG(0x100C);
pub const GENERIC_BATTERY_CLIENT: ModelIdentifier = ModelIdentifier::SIG(0x100D);

pub enum GenericBatteryMessage {
    Get,
    Status(Status),
}

impl Message for GenericBatteryMessage {
    fn opcode(&self) -> Opcode {
        match self {
            Self::Get => GENERIC_BATTERY_GET,
            Self::Status(_) => GENERIC_BATTERY_STATUS,
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut heapless::Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        match self {
            Self::Get => Ok(()),
            Self::Status(inner) => inner.emit_parameters(xmit),
        }
    }
}

impl Model for GenericBatteryServer {
    const IDENTIFIER: ModelIdentifier = GENERIC_BATTERY_SERVER;
    type MESSAGE = GenericBatteryMessage;

    fn parse(
        &self,
        opcode: Opcode,
        _parameters: &[u8],
    ) -> Result<Option<Self::MESSAGE>, ParseError> {
        match opcode {
            GENERIC_BATTERY_GET => Ok(Some(GenericBatteryMessage::Get)),
            _ => Ok(None),
        }
    }
}

opcode!( GENERIC_BATTERY_GET 0x82, 0x23 );
opcode!( GENERIC_BATTERY_STATUS 0x82, 0x24 );

pub struct GenericBatteryFlags {
    pub presence: GenericBatteryFlagsPresence,
    pub indicator: GenericBatteryFlagsIndicator,
    pub charging: GenericBatteryFlagsCharging,
}

impl GenericBatteryFlags {
    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        let mut value: u8 = 0;
        value |= match self.presence {
            GenericBatteryFlagsPresence::NotPresent => 0b00,
            GenericBatteryFlagsPresence::PresentRemovable => 0b01,
            GenericBatteryFlagsPresence::PresentNotRemovable => 0b10,
            GenericBatteryFlagsPresence::Unknown => 0b11,
        } << 4;

        value |= match self.indicator {
            GenericBatteryFlagsIndicator::LowCritical => 0b00,
            GenericBatteryFlagsIndicator::Low => 0b01,
            GenericBatteryFlagsIndicator::Good => 0b10,
            GenericBatteryFlagsIndicator::Unknown => 0b11,
        } << 2;

        value |= match self.charging {
            GenericBatteryFlagsCharging::NotChargeable => 0b00,
            GenericBatteryFlagsCharging::ChargeableNotCharging => 0b01,
            GenericBatteryFlagsCharging::ChargeableCharging => 0b10,
            GenericBatteryFlagsCharging::Unknown => 0b11,
        };

        xmit.push(value).map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}

pub enum GenericBatteryFlagsPresence {
    NotPresent,
    PresentRemovable,
    PresentNotRemovable,
    Unknown,
}

pub enum GenericBatteryFlagsIndicator {
    LowCritical,
    Low,
    Good,
    Unknown,
}

pub enum GenericBatteryFlagsCharging {
    NotChargeable,
    ChargeableNotCharging,
    ChargeableCharging,
    Unknown,
}

pub struct Status {
    battery_level: u8,
    time_to_discharge: u32,
    time_to_charge: u32,
    flags: GenericBatteryFlags,
}

impl Status {
    pub fn new(
        battery_level: u8,
        time_to_discharge: u32,
        time_to_charge: u32,
        flags: GenericBatteryFlags,
    ) -> Self {
        Self {
            battery_level,
            time_to_discharge,
            time_to_charge,
            flags,
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.push(self.battery_level)
            .map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.time_to_discharge.to_be_bytes()[1..])
            .map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.time_to_charge.to_be_bytes()[1..])
            .map_err(|_| InsufficientBuffer)?;
        self.flags.emit_parameters(xmit)?;
        Ok(())
    }
}
