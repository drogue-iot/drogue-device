use crate::traits::lora::*;
use core::fmt::Write;
use heapless::String;

#[derive(Debug)]
pub enum ConfigKey {
    DevAddr,
    DevEui,
    AppEui,
    AppKey,
    NwksKey,
    AppsKey,
    ChMask,
    ChList,
}

#[derive(Debug)]
pub enum Command<'a> {
    QueryFirmwareInfo,
    SetBand(LoraRegion),
    SetMode(LoraMode),
    GetBand,
    Reset(ResetMode),
    Join(ConnectMode),
    SetConfig(ConfigOption<'a>),
    GetConfig(ConfigKey),
    Send(QoS, Port, &'a [u8]),
    GetStatus,
}

#[derive(Debug)]
pub enum ConfigOption<'a> {
    DevAddr(&'a DevAddr),
    DevEui(&'a EUI),
    AppEui(&'a EUI),
    AppKey(&'a AppKey),
    NwksKey(&'a NwksKey),
    AppsKey(&'a AppsKey),
    ChMask(u8, u16),
    /*
    PwrLevel,
    Adr,
    Dr,
    PublicNet,
    RxDelay1,
    Rx2,
    ChList,
    ChMask,
    MaxChs,
    JoinCnt,
    Nbtrans,
    Class,
    Duty,*/
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum Response {
    None,
    Ok,
    Error(i8),
    FirmwareInfo(FirmwareInfo),
    LoraBand(LoraRegion),
    Recv(EventCode, Port, usize, Option<[u8; super::RECV_BUFFER_LEN]>),
    Status {
        tx_ok: u8,
        tx_err: u8,
        rx_ok: u8,
        rx_timeout: u8,
        rx_err: u8,
        rssi: i8,
        snr: u32,
    },
    Initialized(LoraRegion),
}

#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum EventCode {
    RecvData,
    TxConfirmed,
    TxUnconfirmed,
    JoinedSuccess,
    JoinedFailed,
    TxTimeout,
    Rx2Timeout,
    DownlinkRepeated,
    WakeUp,
    P2PTxComplete,
    Unknown,
}

/// Version information for the RAK811 board
#[derive(Debug)]
pub struct FirmwareInfo {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub build: u8,
}

pub type CommandBuffer = String<128>;

impl<'a> Command<'a> {
    pub fn buffer() -> CommandBuffer {
        String::new()
    }

    pub fn encode(&self, s: &mut CommandBuffer) {
        match self {
            Command::QueryFirmwareInfo => {
                write!(s, "at+version").unwrap();
            }
            Command::SetBand(region) => {
                write!(s, "at+band=").unwrap();
                region.encode(s);
            }
            Command::GetBand => {
                write!(s, "at+band").unwrap();
            }
            Command::SetMode(mode) => {
                write!(s, "at+mode=").unwrap();
                mode.encode(s);
            }
            Command::Join(mode) => {
                write!(s, "at+join=").unwrap();
                mode.encode(s);
            }
            Command::SetConfig(opt) => {
                write!(s, "at+set_config=").unwrap();
                opt.encode(s);
            }
            Command::GetConfig(key) => {
                write!(s, "at+get_config=").unwrap();
                key.encode(s);
            }
            Command::Reset(mode) => {
                write!(
                    s,
                    "at+reset={}",
                    match mode {
                        ResetMode::Restart => 0,
                        ResetMode::Reload => 1,
                    }
                )
                .unwrap();
            }
            Command::Send(qos, port, data) => {
                write!(
                    s,
                    "at+send={},{},{}",
                    match qos {
                        QoS::Unconfirmed => 0,
                        QoS::Confirmed => 1,
                    },
                    port,
                    HexSlice(data),
                )
                .unwrap();
            }
            Command::GetStatus => {
                write!(s, "at+status").unwrap();
            }
        }
    }
}

struct HexSlice<'a>(&'a [u8]);

impl<'a> core::fmt::Display for HexSlice<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::result::Result<(), core::fmt::Error> {
        for b in self.0.iter() {
            write!(f, "{:x}", b)?;
        }
        Ok(())
    }
}

impl ConfigKey {
    pub fn encode(&self, s: &mut CommandBuffer) {
        match self {
            ConfigKey::DevAddr => {
                s.push_str("dev_addr").unwrap();
            }
            ConfigKey::DevEui => {
                s.push_str("dev_eui").unwrap();
            }
            ConfigKey::AppEui => {
                s.push_str("app_eui").unwrap();
            }
            ConfigKey::AppKey => {
                s.push_str("app_key").unwrap();
            }
            ConfigKey::NwksKey => {
                s.push_str("nwks_key").unwrap();
            }
            ConfigKey::AppsKey => {
                s.push_str("apps_key").unwrap();
            }
            ConfigKey::ChMask => {
                s.push_str("ch_mask").unwrap();
            }
            ConfigKey::ChList => {
                s.push_str("ch_list").unwrap();
            }
        }
    }
}

impl<'a> ConfigOption<'a> {
    pub fn encode(&self, s: &mut CommandBuffer) {
        match self {
            ConfigOption::DevAddr(addr) => {
                write!(s, "dev_addr:{}", addr).unwrap();
            }
            ConfigOption::DevEui(eui) => {
                write!(s, "dev_eui:{}", eui,).unwrap();
            }
            ConfigOption::AppEui(eui) => {
                write!(s, "app_eui:{}", eui,).unwrap();
            }
            ConfigOption::AppKey(key) => {
                write!(s, "app_key:{}", key).unwrap();
            }
            ConfigOption::NwksKey(key) => {
                write!(s, "nwks_key:{}", key,).unwrap();
            }
            ConfigOption::AppsKey(key) => {
                write!(s, "apps_key:{}", key,).unwrap();
            }
            ConfigOption::ChMask(id, mask) => {
                write!(s, "ch_mask:{},{:04x}", id, mask).unwrap();
            }
        }
    }
}

pub trait Encoder {
    fn encode(&self, s: &mut CommandBuffer);
}

pub trait Decoder {
    fn decode(d: &[u8]) -> Self;
}

impl Encoder for ConnectMode {
    fn encode(&self, s: &mut CommandBuffer) {
        let val = match self {
            ConnectMode::OTAA => "otaa",
            ConnectMode::ABP => "abp",
        };
        s.push_str(val).unwrap();
    }
}

impl Decoder for ConnectMode {
    fn decode(d: &[u8]) -> ConnectMode {
        if let Ok(s) = core::str::from_utf8(d) {
            match s {
                "abp" => ConnectMode::ABP,
                _ => ConnectMode::OTAA,
            }
        } else {
            ConnectMode::OTAA
        }
    }
}

impl Encoder for LoraMode {
    fn encode(&self, s: &mut CommandBuffer) {
        let val = match self {
            LoraMode::WAN => "0",
            LoraMode::P2P => "1",
        };
        s.push_str(val).unwrap();
    }
}

impl Decoder for LoraMode {
    fn decode(d: &[u8]) -> LoraMode {
        if let Ok(s) = core::str::from_utf8(d) {
            match s {
                "1" => LoraMode::P2P,
                _ => LoraMode::WAN,
            }
        } else {
            LoraMode::WAN
        }
    }
}

impl Encoder for LoraRegion {
    fn encode(&self, s: &mut CommandBuffer) {
        let val = match self {
            LoraRegion::EU868 => "EU868",
            LoraRegion::CN470 => "CN470",
            LoraRegion::US915 => "US915",
            LoraRegion::AU915 => "AU915",
            LoraRegion::KR920 => "KR920",
            LoraRegion::AS923 => "AS923",
            LoraRegion::IN865 => "IN865",
            LoraRegion::UNKNOWN => "UNKNOWN",
        };
        s.push_str(val).unwrap();
    }
}

impl Decoder for LoraRegion {
    fn decode(d: &[u8]) -> LoraRegion {
        if let Ok(s) = core::str::from_utf8(d) {
            match s {
                "EU868" => LoraRegion::EU868,
                "US915" => LoraRegion::US915,
                "AU915" => LoraRegion::AU915,
                "KR920" => LoraRegion::KR920,
                "AS923" => LoraRegion::AS923,
                "IN865" => LoraRegion::IN865,
                _ => LoraRegion::UNKNOWN,
            }
        } else {
            LoraRegion::UNKNOWN
        }
    }
}

impl EventCode {
    pub fn parse(d: u8) -> EventCode {
        match d {
            0 => EventCode::RecvData,
            1 => EventCode::TxConfirmed,
            2 => EventCode::TxUnconfirmed,
            3 => EventCode::JoinedSuccess,
            4 => EventCode::JoinedFailed,
            5 => EventCode::TxTimeout,
            6 => EventCode::Rx2Timeout,
            7 => EventCode::DownlinkRepeated,
            8 => EventCode::WakeUp,
            9 => EventCode::P2PTxComplete,
            _ => EventCode::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
