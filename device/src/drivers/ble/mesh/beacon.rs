use crate::drivers::ble::mesh::device::Uuid;

pub enum Beacon {
    Unprovisioned {
        uuid: Uuid,
        oob: OobInformation,
        uri_hash: Option<[u8; 4]>,
    },

    SecureNetwork {
        flags: Flags,
        network_id: [u8; 8],
        iv_index: u32,
        authentication_value: [u8; 8],
    },
}

pub struct OobInformation {
    pub other: bool,
    pub electronic_url: bool,
    pub two_dimensional_machine_readable_code: bool,
    pub bar_code: bool,
    pub nfc: bool,
    pub number: bool,
    pub string: bool,
    pub on_box: bool,
    pub inside_box: bool,
    pub on_piece_of_paper: bool,
    pub inside_manual: bool,
    pub on_device: bool,
}

pub struct Flags {
    key_refresh: bool,
    iv_update: bool,
}
