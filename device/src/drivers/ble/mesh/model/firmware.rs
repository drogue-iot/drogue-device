use crate::drivers::ble::mesh::model::{CompanyIdentifier, Message, Model, ModelIdentifier};
use crate::drivers::ble::mesh::pdu::access::Opcode;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use crate::opcode;

pub struct FirmwareUpdateClient;
pub struct FirmwareUpdateServer;

const COMPANY_IDENTIFIER: CompanyIdentifier = CompanyIdentifier(0x0003);

pub const FIRMWARE_UPDATE_CLIENT: ModelIdentifier =
    ModelIdentifier::Vendor(COMPANY_IDENTIFIER, 0x11ed);
pub const FIRMWARE_UPDATE_SERVER: ModelIdentifier =
    ModelIdentifier::Vendor(COMPANY_IDENTIFIER, 0x11ec);

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum FirmwareUpdateMessage<'m> {
    Get,
    Status(Status<'m>),
    Control(Control<'m>),
    Write(Write<'m>),
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Write<'m> {
    pub offset: u32,
    pub payload: &'m [u8],
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct Status<'m> {
    pub mtu: u8,
    pub offset: u32,
    pub version: &'m [u8],
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Control<'m> {
    Start,
    Update,
    MarkBooted,
    NextVersion(&'m [u8]),
}

opcode!( FIRMWARE_UPDATE_GET 0xF0, 0x03, 0x00);
opcode!( FIRMWARE_UPDATE_STATUS 0xF1, 0x03, 0x00);
opcode!( FIRMWARE_UPDATE_CONTROL 0xF2, 0x03, 0x00);
opcode!( FIRMWARE_UPDATE_WRITE 0xF3, 0x03, 0x00);

impl<'m> Message for FirmwareUpdateMessage<'m> {
    fn opcode(&self) -> Opcode {
        match self {
            Self::Get => FIRMWARE_UPDATE_GET,
            Self::Status(_) => FIRMWARE_UPDATE_STATUS,
            Self::Control(_) => FIRMWARE_UPDATE_CONTROL,
            Self::Write(_) => FIRMWARE_UPDATE_WRITE,
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut heapless::Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        match self {
            Self::Get => Ok(()),
            Self::Status(inner) => inner.emit_parameters(xmit),
            Self::Control(inner) => inner.emit_parameters(xmit),
            Self::Write(inner) => inner.emit_parameters(xmit),
        }
    }
}

impl Model for FirmwareUpdateServer {
    const IDENTIFIER: ModelIdentifier = FIRMWARE_UPDATE_SERVER;
    type Message<'m> = FirmwareUpdateMessage<'m>;

    fn parse<'m>(
        opcode: Opcode,
        parameters: &'m [u8],
    ) -> Result<Option<Self::Message<'m>>, ParseError> {
        match opcode {
            FIRMWARE_UPDATE_GET => Ok(Some(FirmwareUpdateMessage::Get)),
            FIRMWARE_UPDATE_STATUS => Ok(Some(FirmwareUpdateMessage::Status(Status::parse(
                parameters,
            )?))),
            FIRMWARE_UPDATE_CONTROL => Ok(Some(FirmwareUpdateMessage::Control(Control::parse(
                parameters,
            )?))),
            FIRMWARE_UPDATE_WRITE => Ok(Some(FirmwareUpdateMessage::Write(Write::parse(
                parameters,
            )?))),
            _ => Ok(None),
        }
    }
}

impl<'m> Control<'m> {
    fn parse(parameters: &'m [u8]) -> Result<Self, ParseError> {
        let t = parameters[0];
        match t {
            1 => Ok(Control::Start),
            2 => Ok(Control::Update),
            3 => Ok(Control::MarkBooted),
            4 => {
                let len = parameters[1] as usize;
                let version = &parameters[2..2 + len];
                Ok(Control::NextVersion(version))
            }
            _ => Err(ParseError::InvalidLength),
        }
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut heapless::Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.push(match self {
            Control::Start => 1,
            Control::Update => 2,
            Control::MarkBooted => 3,
            Control::NextVersion(_) => 4,
        })
        .map_err(|_| InsufficientBuffer)?;

        if let Control::NextVersion(version) = self {
            xmit.push(version.len() as u8)
                .map_err(|_| InsufficientBuffer)?;
            xmit.extend_from_slice(version)
                .map_err(|_| InsufficientBuffer)?;
        }
        Ok(())
    }
}

impl<'m> Status<'m> {
    fn parse(parameters: &'m [u8]) -> Result<Self, ParseError> {
        let mtu = parameters[0];
        let offset =
            u32::from_le_bytes([parameters[1], parameters[2], parameters[3], parameters[4]]);
        let len = parameters[5] as usize;
        let version = &parameters[6..6 + len];
        Ok(Status {
            mtu,
            offset,
            version,
        })
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut heapless::Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.push(self.mtu).map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(&self.offset.to_le_bytes())
            .map_err(|_| InsufficientBuffer)?;
        xmit.push(self.version.len() as u8)
            .map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(self.version)
            .map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}

impl<'m> Write<'m> {
    fn parse(parameters: &'m [u8]) -> Result<Self, ParseError> {
        let offset =
            u32::from_le_bytes([parameters[0], parameters[1], parameters[2], parameters[3]]);
        let len = parameters[4] as usize;
        let payload = &parameters[5..5 + len];
        Ok(Write { offset, payload })
    }

    fn emit_parameters<const N: usize>(
        &self,
        xmit: &mut heapless::Vec<u8, N>,
    ) -> Result<(), InsufficientBuffer> {
        xmit.extend_from_slice(&self.offset.to_le_bytes())
            .map_err(|_| InsufficientBuffer)?;
        xmit.push(self.payload.len() as u8)
            .map_err(|_| InsufficientBuffer)?;
        xmit.extend_from_slice(self.payload)
            .map_err(|_| InsufficientBuffer)?;
        Ok(())
    }
}
