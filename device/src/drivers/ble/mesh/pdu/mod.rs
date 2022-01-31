use defmt::Format;

pub mod access;
pub mod bearer;
pub mod lower;
pub mod network;
pub mod upper;

#[derive(Format)]
pub enum ParseError {
    InvalidPDUFormat,
    InvalidValue,
    InvalidLength,
    InsufficientBuffer,
}
