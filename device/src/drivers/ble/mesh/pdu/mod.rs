use defmt::Format;

pub mod access;
pub mod network;
pub mod lower;
pub mod upper;
pub mod bearer;

#[derive(Format)]
pub enum ParseError {
    InvalidPDUFormat,
    InvalidValue,
    InvalidLength,
    InsufficientBuffer,
}
