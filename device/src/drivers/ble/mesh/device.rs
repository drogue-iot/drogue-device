use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Copy, Clone, Eq, PartialEq)]
pub struct Uuid(pub [u8; 16]);

#[cfg(feature = "defmt")]
impl defmt::Format for Uuid {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}{=u8:02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7], self.0[8], self.0[9], self.0[10], self.0[11], self.0[12], self.0[13], self.0[14], self.0[15],
        )
    }
}

pub struct Device {
    uuid: Uuid,
    state: DeviceState,
}

pub enum DeviceState {
    Unprovisioned,
    Node,
}

impl Default for DeviceState {
    fn default() -> Self {
        DeviceState::Unprovisioned
    }
}
