pub mod access;
pub mod bearer;
pub mod lower;
pub mod network;
pub mod upper;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum ParseError {
    InvalidPDUFormat,
    InvalidValue,
    InvalidLength,
    InsufficientBuffer,
}
