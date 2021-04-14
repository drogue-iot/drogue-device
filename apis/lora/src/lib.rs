#![no_std]

#[derive(Debug, Clone, Copy)]
pub enum QoS {
    Unconfirmed,
    Confirmed,
}

#[derive(Debug, Clone, Copy)]
pub enum ResetMode {
    Restart,
    Reload,
}

#[derive(Debug, Clone, Copy)]
pub enum ConnectMode {
    OTAA,
    ABP,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
pub enum LoraMode {
    WAN = 0,
    P2P = 1,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LoraRegion {
    EU868,
    US915,
    AU915,
    KR920,
    AS923,
    IN865,
    UNKNOWN,
}

pub type Port = u8;
#[derive(Debug, Clone, Copy)]
pub struct DevAddr([u8; 4]);
#[derive(Debug, Clone, Copy)]
pub struct EUI([u8; 8]);
#[derive(Debug, Clone, Copy)]
pub struct AppKey([u8; 16]);
#[derive(Debug, Clone, Copy)]
pub struct NwksKey([u8; 16]);
#[derive(Debug, Clone, Copy)]
pub struct AppsKey([u8; 16]);

#[derive(Debug, Clone, Copy)]
pub struct LoraConfig {
    pub band: Option<LoraRegion>,
    pub lora_mode: Option<LoraMode>,
    pub device_address: Option<DevAddr>,
    pub device_eui: Option<EUI>,
    pub app_eui: Option<EUI>,
    pub app_key: Option<AppKey>,
}

impl LoraConfig {
    pub fn new() -> Self {
        Self {
            band: None,
            lora_mode: None,
            device_address: None,
            device_eui: None,
            app_eui: None,
            app_key: None,
        }
    }

    pub fn band(mut self, band: LoraRegion) -> Self {
        self.band.replace(band);
        self
    }

    pub fn lora_mode(mut self, lora_mode: LoraMode) -> Self {
        self.lora_mode.replace(lora_mode);
        self
    }

    pub fn device_address(mut self, device_address: &DevAddr) -> Self {
        self.device_address.replace(device_address.clone());
        self
    }

    pub fn device_eui(mut self, device_eui: &EUI) -> Self {
        self.device_eui.replace(device_eui.clone());
        self
    }

    pub fn app_eui(mut self, app_eui: &EUI) -> Self {
        self.app_eui.replace(app_eui.clone());
        self
    }

    pub fn app_key(mut self, app_key: &AppKey) -> Self {
        self.app_key.replace(app_key.clone());
        self
    }
}

impl EUI {
    pub fn reverse(&self) -> Self {
        let mut idx = 0;
        let mut output: [u8; 8] = self.0;
        let end = output.len();
        while idx < end / 2 {
            output[idx] = self.0[end - idx - 1];
            output[end - idx - 1] = self.0[idx];
            idx += 1;
        }
        Self(output)
    }
}

impl core::convert::From<&str> for EUI {
    fn from(input: &str) -> Self {
        assert!(input.len() >= 16);
        let mut b = [0; 8];
        for i in 0..b.len() {
            b[i] = u8::from_str_radix(&input[(i * 2)..(i * 2) + 2], 16).unwrap();
        }
        EUI(b)
    }
}

impl core::fmt::Display for EUI {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5], self.0[6], self.0[7]
        )
    }
}

impl core::convert::From<[u8; 8]> for EUI {
    fn from(input: [u8; 8]) -> Self {
        Self(input)
    }
}

impl core::convert::From<EUI> for [u8; 8] {
    fn from(input: EUI) -> Self {
        input.0
    }
}

impl DevAddr {
    pub fn reverse(&self) -> Self {
        let mut idx = 0;
        let mut output: [u8; 4] = self.0;
        let end = output.len();
        while idx < end / 2 {
            output[idx] = self.0[end - idx - 1];
            output[end - idx - 1] = self.0[idx];
            idx += 1;
        }
        Self(output)
    }
}

impl core::convert::From<&str> for DevAddr {
    fn from(input: &str) -> Self {
        assert!(input.len() >= 8);
        let mut b = [0; 4];
        for i in 0..b.len() {
            b[i] = u8::from_str_radix(&input[(i * 2)..(i * 2) + 2], 16).unwrap();
        }
        DevAddr(b)
    }
}

impl core::fmt::Display for DevAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3]
        )
    }
}

impl core::convert::From<[u8; 4]> for DevAddr {
    fn from(input: [u8; 4]) -> Self {
        Self(input)
    }
}

impl core::convert::From<DevAddr> for [u8; 4] {
    fn from(input: DevAddr) -> Self {
        input.0
    }
}

fn reverse_16(s: &[u8; 16]) -> [u8; 16] {
    let mut idx = 0;
    let mut output: [u8; 16] = s.clone();
    let end = output.len();
    while idx < end / 2 {
        output[idx] = s[end - idx - 1];
        output[end - idx - 1] = s[idx];
        idx += 1;
    }
    output
}

impl AppKey {
    pub fn reverse(&self) -> Self {
        Self(reverse_16(&self.0))
    }
}

impl core::convert::From<&str> for AppKey {
    fn from(input: &str) -> Self {
        assert!(input.len() >= 32);
        let mut b = [0; 16];
        for i in 0..b.len() {
            b[i] = u8::from_str_radix(&input[(i * 2)..(i * 2) + 2], 16).unwrap();
        }
        AppKey(b)
    }
}

impl core::fmt::Display for AppKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3],
            self.0[4], self.0[5], self.0[6], self.0[7],
            self.0[8], self.0[9], self.0[10], self.0[11],
            self.0[12], self.0[13], self.0[14], self.0[15],
        )
    }
}

impl core::convert::From<[u8; 16]> for AppKey {
    fn from(input: [u8; 16]) -> Self {
        Self(input)
    }
}

impl core::convert::From<AppKey> for [u8; 16] {
    fn from(input: AppKey) -> Self {
        input.0
    }
}

impl NwksKey {
    pub fn reverse(&self) -> Self {
        Self(reverse_16(&self.0))
    }
}

impl core::convert::From<&str> for NwksKey {
    fn from(input: &str) -> Self {
        assert!(input.len() >= 32);
        let mut b = [0; 16];
        for i in 0..b.len() {
            b[i] = u8::from_str_radix(&input[(i * 2)..(i * 2) + 2], 16).unwrap();
        }
        NwksKey(b)
    }
}

impl core::fmt::Display for NwksKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3],
            self.0[4], self.0[5], self.0[6], self.0[7],
            self.0[8], self.0[9], self.0[10], self.0[11],
            self.0[12], self.0[13], self.0[14], self.0[15],
        )
    }
}

impl core::convert::From<[u8; 16]> for NwksKey {
    fn from(input: [u8; 16]) -> Self {
        Self(input)
    }
}

impl core::convert::From<NwksKey> for [u8; 16] {
    fn from(input: NwksKey) -> Self {
        input.0
    }
}

impl AppsKey {
    pub fn reverse(&self) -> Self {
        Self(reverse_16(&self.0))
    }
}

impl core::convert::From<&str> for AppsKey {
    fn from(input: &str) -> Self {
        assert!(input.len() >= 32);
        let mut b = [0; 16];
        for i in 0..b.len() {
            b[i] = u8::from_str_radix(&input[(i * 2)..(i * 2) + 2], 16).unwrap();
        }
        AppsKey(b)
    }
}

impl core::fmt::Display for AppsKey {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.0[0], self.0[1], self.0[2], self.0[3],
            self.0[4], self.0[5], self.0[6], self.0[7],
            self.0[8], self.0[9], self.0[10], self.0[11],
            self.0[12], self.0[13], self.0[14], self.0[15],
        )
    }
}

impl core::convert::From<[u8; 16]> for AppsKey {
    fn from(input: [u8; 16]) -> Self {
        Self(input)
    }
}

impl core::convert::From<AppsKey> for [u8; 16] {
    fn from(input: AppsKey) -> Self {
        input.0
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    #[test]
    fn test_conversion() {
        let s = "AABBCCDDEEFF0011";
        let eui: EUI = s.into();
        let data: [u8; 8] = eui.into();

        assert_eq!(data[0], 0xAA);
        assert_eq!(data[1], 0xBB);
        assert_eq!(data[2], 0xCC);
        assert_eq!(data[3], 0xDD);
        assert_eq!(data[4], 0xEE);
        assert_eq!(data[5], 0xFF);
        assert_eq!(data[6], 0x00);
        assert_eq!(data[7], 0x11);
    }

    #[test]
    fn test_reverse() {
        let s = "AABBCCDDEEFF0011";
        let eui: EUI = s.into();
        let reversed: [u8; 8] = eui.reverse().into();
        assert_eq!(0x11, reversed[0]);
        assert_eq!(0x00, reversed[1]);
        assert_eq!(0xFF, reversed[2]);
        assert_eq!(0xEE, reversed[3]);
        assert_eq!(0xDD, reversed[4]);
        assert_eq!(0xCC, reversed[5]);
        assert_eq!(0xBB, reversed[6]);
        assert_eq!(0xAA, reversed[7]);
    }
}
