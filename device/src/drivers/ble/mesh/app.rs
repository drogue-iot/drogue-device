use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, PartialEq, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct ApplicationKeyIdentifier(u8);

impl From<u8> for ApplicationKeyIdentifier {
    fn from(val: u8) -> Self {
        Self(val)
    }
}

impl From<ApplicationKeyIdentifier> for u8 {
    fn from(val: ApplicationKeyIdentifier) -> Self {
        val.0
    }
}
