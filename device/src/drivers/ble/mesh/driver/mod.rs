use crate::drivers::ble::mesh::interface::NetworkError;
use crate::drivers::ble::mesh::model::Status;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use cmac::crypto_mac::InvalidKeyLength;
use postcard::Error;

pub mod elements;
pub mod node;
mod pipeline;

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DeviceError {
    PipelineNotConfigured,
    AlreadyProvisioned,
    CryptoError(&'static str),
    Storage,
    StorageInitialization,
    KeyInitialization,
    InvalidPacket,
    InsufficientBuffer,
    InvalidLink,
    NoEstablishedLink,
    InvalidKeyLength,
    InvalidTransactionNumber,
    IncompleteTransaction,
    NoSharedSecret,
    ParseError(ParseError),
    TransmitError,
    Serialization,
    InvalidSrcAddress,
    InvalidDstAddress,
    InvalidState,
    NotProvisioned,
    Status(Status),
    Network(NetworkError),
}

impl From<NetworkError> for DeviceError {
    fn from(err: NetworkError) -> Self {
        Self::Network(err)
    }
}

impl From<Status> for DeviceError {
    fn from(status: Status) -> Self {
        Self::Status(status)
    }
}

impl From<InvalidKeyLength> for DeviceError {
    fn from(_: InvalidKeyLength) -> Self {
        DeviceError::InvalidKeyLength
    }
}

impl From<ParseError> for DeviceError {
    fn from(inner: ParseError) -> Self {
        DeviceError::ParseError(inner)
    }
}

impl From<InsufficientBuffer> for DeviceError {
    fn from(_: InsufficientBuffer) -> Self {
        DeviceError::InsufficientBuffer
    }
}

impl From<postcard::Error> for DeviceError {
    fn from(_: Error) -> Self {
        DeviceError::Serialization
    }
}
