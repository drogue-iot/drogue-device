use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
use crate::drivers::ble::mesh::app::ApplicationKeyIdentifier;
use crate::drivers::ble::mesh::config::network::NetworkKeyHandle;
use crate::drivers::ble::mesh::driver::elements::ElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::Message;
use crate::drivers::ble::mesh::pdu::upper::UpperAccess;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use heapless::Vec;

#[derive(Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AccessMessage {
    pub ttl: Option<u8>,
    pub(crate) network_key: NetworkKeyHandle,
    pub(crate) ivi: u8,
    pub(crate) nid: u8,
    pub(crate) akf: bool,
    pub(crate) aid: ApplicationKeyIdentifier,
    pub(crate) src: UnicastAddress,
    pub(crate) dst: Address,
    pub(crate) payload: AccessPayload,
}

#[allow(unused)]
impl AccessMessage {
    pub fn with_ttl(mut self, ttl: u8) -> Self {
        self.ttl.replace(ttl);
        self
    }

    pub fn opcode(&self) -> Opcode {
        self.payload.opcode
    }

    pub fn parameters(&self) -> &[u8] {
        &self.payload.parameters
    }

    pub fn parse(access: &UpperAccess) -> Result<Self, ParseError> {
        Ok(Self {
            ttl: None,
            network_key: access.network_key,
            ivi: access.ivi,
            nid: access.nid,
            akf: access.akf,
            aid: access.aid,
            src: access.src,
            dst: access.dst,
            payload: AccessPayload::parse(&access.payload)?,
        })
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        self.payload.emit(xmit)
    }

    pub fn create_response<C: ElementContext, M: Message>(
        &self,
        ctx: &C,
        response: M,
    ) -> Result<Self, DeviceError> {
        let mut parameters = Vec::new();
        response
            .emit_parameters(&mut parameters)
            .map_err(|_| InsufficientBuffer)?;
        Ok(Self {
            ttl: None,
            src: ctx.address().ok_or(DeviceError::NotProvisioned)?,
            dst: self.src.into(),
            payload: AccessPayload {
                opcode: response.opcode(),
                parameters,
            },
            ..*self
        })
    }
}

#[derive(Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct AccessPayload {
    pub opcode: Opcode,
    pub parameters: Vec<u8, 384>,
}

#[allow(unused)]
impl AccessPayload {
    pub fn parse(data: &[u8]) -> Result<Self, ParseError> {
        let (opcode, parameters) = Opcode::split(data).ok_or(ParseError::InvalidPDUFormat)?;
        Ok(Self {
            opcode,
            parameters: Vec::from_slice(parameters).map_err(|_| ParseError::InsufficientBuffer)?,
        })
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        self.opcode.emit(xmit)?;
        xmit.extend_from_slice(&self.parameters)
            .map_err(|_| InsufficientBuffer)
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Opcode {
    OneOctet(u8),
    TwoOctet(u8, u8),
    ThreeOctet(u8, u8, u8),
}

#[allow(unused)]
#[cfg(feature = "defmt")]
impl defmt::Format for Opcode {
    fn format(&self, fmt: defmt::Formatter) {
        match self {
            Opcode::OneOctet(a) => {
                defmt::write!(fmt, "{:02x}", a)
            }
            Opcode::TwoOctet(a, b) => {
                defmt::write!(fmt, "{:02x}{:02x}", a, b)
            }
            Opcode::ThreeOctet(a, b, c) => {
                defmt::write!(fmt, "{:02x}{:02x}{:02x}", a, b, c)
            }
        }
    }
}

impl Opcode {
    pub fn matches(&self, data: &[u8]) -> bool {
        match self {
            Opcode::OneOctet(a) if data.len() >= 1 && data[0] == *a => true,
            Opcode::TwoOctet(a, b) if data.len() >= 2 && data[0] == *a && data[1] == *b => true,
            Opcode::ThreeOctet(a, b, c)
                if data.len() >= 3 && data[0] == *a && data[1] == *b && data[2] == *c =>
            {
                true
            }
            _ => false,
        }
    }

    pub fn opcode_len(&self) -> usize {
        match self {
            Opcode::OneOctet(_) => 1,
            Opcode::TwoOctet(_, _) => 2,
            Opcode::ThreeOctet(_, _, _) => 3,
        }
    }

    pub fn split(data: &[u8]) -> Option<(Opcode, &[u8])> {
        if data.is_empty() {
            None
        } else {
            if data[0] & 0b10000000 == 0 {
                // one octet
                Some((Opcode::OneOctet(data[0] & 0b00111111), &data[1..]))
            } else if data.len() >= 2 && data[0] & 0b11000000 == 0b10000000 {
                // two octet
                Some((Opcode::TwoOctet(data[0], data[1]), &data[2..]))
            } else if data.len() >= 3 && data[0] & 0b11000000 == 0b11000000 {
                // three octet
                Some((Opcode::ThreeOctet(data[0], data[1], data[2]), &data[3..]))
            } else {
                None
            }
        }
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        match self {
            Opcode::OneOctet(a) => {
                xmit.push(*a).map_err(|_| InsufficientBuffer)?;
            }
            Opcode::TwoOctet(a, b) => {
                xmit.push(*a).map_err(|_| InsufficientBuffer)?;
                xmit.push(*b).map_err(|_| InsufficientBuffer)?;
            }
            Opcode::ThreeOctet(a, b, c) => {
                xmit.push(*a).map_err(|_| InsufficientBuffer)?;
                xmit.push(*b).map_err(|_| InsufficientBuffer)?;
                xmit.push(*c).map_err(|_| InsufficientBuffer)?;
            }
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! opcode {
    ($name:ident $o1:expr) => {
        pub const $name: Opcode = Opcode::OneOctet($o1);
    };

    ($name:ident $o1:expr, $o2:expr) => {
        pub const $name: Opcode = Opcode::TwoOctet($o1, $o2);
    };

    ($name:ident $o1:expr, $o2:expr, $o3:expr) => {
        pub const $name: Opcode = Opcode::ThreeOctet($o1, $o2, $o3);
    };
}

opcode!( CONFIG_BEACON_GET 0x80, 0x09 );
opcode!( CONFIG_BEACON_SET 0x80, 0x0A );
opcode!( CONFIG_BEACON_STATUS 0x80, 0x0B );
opcode!( CONFIG_FRIEND_GET 0x80, 0x0F );
opcode!( CONFIG_FRIEND_SET 0x80, 0x10 );
opcode!( CONFIG_FRIEND_STATUS 0x80, 0x11 );
opcode!( CONFIG_GATT_PROXY_GET 0x80, 0x12 );
opcode!( CONFIG_GATT_PROXY_SET 0x80, 0x13 );
opcode!( CONFIG_GATT_PROXY_STATUS 0x80, 0x14 );
opcode!( CONFIG_HEARTBEAT_PUBLICATION_GET 0x80, 0x38 );
opcode!( CONFIG_HEARTBEAT_PUBLICATION_SET 0x80, 0x39 );
opcode!( CONFIG_HEARTBEAT_PUBLICATION_STATUS 0x06 );
opcode!( CONFIG_HEARTBEAT_SUBSCRIPTION_GET 0x80, 0x3A );
opcode!( CONFIG_HEARTBEAT_SUBSCRIPTION_SET 0x80, 0x3B );
opcode!( CONFIG_HEARTBEAT_SUBSCRIPTION_STATUS 0x80, 0x3C );
opcode!( CONFIG_KEY_REFRESH_PHASE_GET 0x80, 0x15 );
opcode!( CONFIG_KEY_REFRESH_PHASE_SET 0x80, 0x16 );
opcode!( CONFIG_KEY_REFRESH_PHASE_STATUS 0x80, 0x17 );
opcode!( CONFIG_LOW_POWER_NODE_POLLTIMEOUT_GET 0x80, 0x2D );
opcode!( CONFIG_LOW_POWER_NODE_POLLTIMEOUT_STATUS 0x80, 0x2E );
opcode!( CONFIG_NETKEY_ADD 0x80, 0x40);
opcode!( CONFIG_NETKEY_DELETE 0x80, 0x41);
opcode!( CONFIG_NETKEY_GET 0x80, 0x42);
opcode!( CONFIG_NETKEY_LIST 0x80, 0x43);
opcode!( CONFIG_NETKEY_STATUS 0x80, 0x44);
opcode!( CONFIG_NETKEY_UPDATE 0x80, 0x45);
opcode!( CONFIG_NETWORK_TRANSMIT_GET 0x80, 0x23);
opcode!( CONFIG_NETWORK_TRANSMIT_SET 0x80, 0x24);
opcode!( CONFIG_NETWORK_TRANSMIT_STATUS 0x80, 0x25);
opcode!( CONFIG_NODE_IDENTITY_GET 0x80, 0x46);
opcode!( CONFIG_NODE_IDENTITY_SET 0x80, 0x47);
opcode!( CONFIG_NODE_IDENTITY_STATUS 0x80, 0x48);
opcode!( CONFIG_RELAY_GET 0x80, 0x26);
opcode!( CONFIG_RELAY_SET 0x80, 0x27);
opcode!( CONFIG_RELAY_STATUS 0x80, 0x28);
opcode!( CONFIG_SIG_MODEL_APP_GET 0x80, 0x4B);
opcode!( CONFIG_SIG_MODEL_APP_LIST 0x80, 0x4C);
opcode!( CONFIG_SIG_MODEL_SUBSCRIPTION_GET 0x80, 0x29);
opcode!( CONFIG_SIG_MODEL_SUBSCRIPTION_LIST 0x80, 0x2A );
opcode!( CONFIG_VENDOR_MODEL_APP_GET 0x80, 0x4D );
opcode!( CONFIG_VENDOR_MODEL_APP_LIST 0x80, 0x4E );
opcode!( CONFIG_VENDOR_MODEL_SUBSCRIPTION_GET 0x80, 0x2B );
opcode!( CONFIG_VENDOR_MODEL_SUBSCRIPTION_LIST 0x80, 0x2C );

opcode!( HEALTH_ATTENTION_GET 0x80, 0x04 );
opcode!( HEALTH_ATTENTION_SET 0x80, 0x05 );
opcode!( HEALTH_ATTENTION_SET_UNACKNOWLEDGED 0x80, 0x06 );
opcode!( HEALTH_ATTENTION_STATUS 0x80, 0x07 );
opcode!( HEALTH_CURRENT_STATUS 0x04 );
opcode!( HEALTH_FAULT_CLEAR 0x80, 0x2F );
opcode!( HEALTH_FAULT_CLEAR_UNACKNOWLEDGED 0x80, 0x30 );
opcode!( HEALTH_FAULT_GET 0x80, 0x31 );
opcode!( HEALTH_FAULT_STATUS 0x05 );
opcode!( HEALTH_FAULT_TEST 0x80, 0x32 );
opcode!( HEALTH_FAULT_TEST_UNACKNOWLEDGED 0x80, 0x33 );
opcode!( HEALTH_PERIOD_GET 0x80, 0x34 );
opcode!( HEALTH_PERIOD_SET 0x80, 0x35 );
opcode!( HEALTH_PERIOD_SET_UNACKNOWLEDGED 0x80, 0x36 );
opcode!( HEALTH_PERIOD_STATUS 0x80, 0x37 );
