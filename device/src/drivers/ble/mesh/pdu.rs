pub mod upper {
    use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
    use heapless::Vec;

    pub struct PDU {
        ivi: bool, /* 1 bit */
        nid: u8,   /* 7 bits */
        ttl: u8,   /* 7 bits */
        seq: u32,  /* 24 bits */
        src: UnicastAddress,
        dst: Address,
        message: PDUMessage,
    }

    pub enum PDUMessage {
        Access {
            transport_pdu: Vec<u8, 16>,
            net_mic: [u8; 4],
        },
        Control {
            transport_pdu: Vec<u8, 12>,
            net_mic: [u8; 8],
        },
    }

    pub enum TransportPDU {
        Control,
        NotControl,
    }

    pub enum NetMIC {
        Control([u8; 4]),
        NotControl([u8; 8]),
    }
}

pub mod lower {
    use crate::drivers::ble::mesh::app::ApplicationKeyIdentifier;
    use crate::drivers::ble::mesh::control::Opcode;
    use heapless::Vec;

    pub enum PDU {
        Access,
        Control,
    }

    pub struct Access {
        akf: bool,
        aid: ApplicationKeyIdentifier,
        message: AccessMessage,
    }

    pub struct Control {
        opcode: Opcode,
        message: ControlMessage,
    }

    pub enum AccessMessage {
        Unsegmented(Vec<u8, 15>),
        Segmented {
            szmic: bool,
            seq_zero: u16,
            seg_o: u8,
            seg_n: u8,
            segment_m: Vec<u8, 12>,
        },
    }

    pub enum ControlMessage {
        Unsegmented {
            parameters: Vec<u8, 11>,
        },
        Segmented {
            seq_zero: u16,
            seg_o: u8,
            seg_n: u8,
            segment_m: Vec<u8, 8>,
        },
    }

    pub struct SegmentAck {
        seq_zero: u16,
        block_ack: u32,
    }
}
