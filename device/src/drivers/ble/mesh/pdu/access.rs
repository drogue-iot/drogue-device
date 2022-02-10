use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
use crate::drivers::ble::mesh::app::ApplicationKeyIdentifier;
use crate::drivers::ble::mesh::configuration_manager::NetworkKeyHandle;
use crate::drivers::ble::mesh::driver::elements::ElementContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::model::Message;
use crate::drivers::ble::mesh::pdu::upper::UpperAccess;
use crate::drivers::ble::mesh::pdu::ParseError;
use crate::drivers::ble::mesh::InsufficientBuffer;
use defmt::{Format, Formatter};
use heapless::Vec;

#[derive(Format)]
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
    ) -> Result<AccessMessage, DeviceError> {
        let mut parameters = Vec::new();
        response
            .emit_parameters(&mut parameters)
            .map_err(|_| InsufficientBuffer)?;
        Ok(AccessMessage {
            ttl: None,
            network_key: self.network_key,
            ivi: self.ivi,
            nid: self.nid,
            akf: self.akf,
            aid: self.aid,
            src: ctx.address().ok_or(DeviceError::NotProvisioned)?,
            dst: self.src.into(),
            payload: AccessPayload {
                opcode: response.opcode(),
                parameters,
            },
        })
    }
}

#[derive(Format)]
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

#[derive(Format)]
pub enum Config {
    Friend(Friend),
    GATTProxy(GATTProxy),
    HeartbeatPublication(HeartbeatPublication),
    HeartbeatSubscription(HeartbeatSubscription),
    KeyRefreshPhase(KeyRefreshPhase),
    LowPowerNodePollTimeout(LowPowerNodePollTimeout),
    Model(Model),
    NetKey(NetKey),
    NetworkTransmit(NetworkTransmit),
    NodeIdentity(NodeIdentity),
    Relay(Relay),
    SIGModel(SIGModel),
    VendorModel(VendorModel),
}

#[allow(unused)]
impl Config {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Friend(inner) => inner.opcode(),
            Self::GATTProxy(inner) => inner.opcode(),
            Self::HeartbeatPublication(inner) => inner.opcode(),
            Self::HeartbeatSubscription(inner) => inner.opcode(),
            Self::KeyRefreshPhase(inner) => inner.opcode(),
            Self::LowPowerNodePollTimeout(inner) => inner.opcode(),
            Self::Model(inner) => inner.opcode(),
            Self::NetKey(inner) => inner.opcode(),
            Self::NetworkTransmit(inner) => inner.opcode(),
            Self::NodeIdentity(inner) => inner.opcode(),
            Self::Relay(inner) => inner.opcode(),
            Self::SIGModel(inner) => inner.opcode(),
            Self::VendorModel(inner) => inner.opcode(),
        }
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        match self {
            Config::Friend(inner) => inner.emit(xmit),
            Config::GATTProxy(inner) => inner.emit(xmit),
            Config::HeartbeatPublication(inner) => inner.emit(xmit),
            Config::HeartbeatSubscription(inner) => inner.emit(xmit),
            Config::KeyRefreshPhase(inner) => inner.emit(xmit),
            Config::LowPowerNodePollTimeout(inner) => inner.emit(xmit),
            Config::Model(inner) => inner.emit(xmit),
            Config::NetKey(inner) => inner.emit(xmit),
            Config::NetworkTransmit(inner) => inner.emit(xmit),
            Config::NodeIdentity(inner) => inner.emit(xmit),
            Config::Relay(inner) => inner.emit(xmit),
            Config::SIGModel(inner) => inner.emit(xmit),
            Config::VendorModel(inner) => inner.emit(xmit),
        }
    }
}

#[derive(Format)]
pub enum Friend {
    Get,
    Set,
    Status,
}

#[allow(unused)]
impl Friend {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_FRIEND_GET,
            Self::Set => CONFIG_FRIEND_SET,
            Self::Status => CONFIG_FRIEND_STATUS,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum GATTProxy {
    Get,
    Set,
    Status,
}

#[allow(unused)]
impl GATTProxy {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_GATT_PROXY_GET,
            Self::Set => CONFIG_GATT_PROXY_SET,
            Self::Status => CONFIG_GATT_PROXY_STATUS,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum HeartbeatPublication {
    Get,
    Set,
    Status,
}

#[allow(unused)]
impl HeartbeatPublication {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_HEARTBEAT_PUBLICATION_GET,
            Self::Set => CONFIG_HEARTBEAT_PUBLICATION_SET,
            Self::Status => CONFIG_HEARTBEAT_PUBLICATION_STATUS,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum HeartbeatSubscription {
    Get,
    Set,
    Status,
}

#[allow(unused)]
impl HeartbeatSubscription {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_HEARTBEAT_SUBSCRIPTION_GET,
            Self::Set => CONFIG_HEARTBEAT_SUBSCRIPTION_SET,
            Self::Status => CONFIG_HEARTBEAT_SUBSCRIPTION_STATUS,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum KeyRefreshPhase {
    Get,
    Set,
    Status,
}

#[allow(unused)]
impl KeyRefreshPhase {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_KEY_REFRESH_PHASE_GET,
            Self::Set => CONFIG_KEY_REFRESH_PHASE_SET,
            Self::Status => CONFIG_KEY_REFRESH_PHASE_STATUS,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum LowPowerNodePollTimeout {
    Get,
    Status,
}

#[allow(unused)]
impl LowPowerNodePollTimeout {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_LOW_POWER_NODE_POLLTIMEOUT_GET,
            Self::Status => CONFIG_LOW_POWER_NODE_POLLTIMEOUT_STATUS,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum Model {
    App(ModelApp),
    Publication(ModelPublication),
    Subscription(ModelSubscription),
}

#[allow(unused)]
impl Model {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::App(inner) => inner.opcode(),
            Self::Publication(inner) => inner.opcode(),
            Self::Subscription(inner) => inner.opcode(),
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum ModelApp {
    Bind,
    Status,
    Unbind,
}

#[allow(unused)]
impl ModelApp {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Bind => CONFIG_MODEL_APP_BIND,
            Self::Status => CONFIG_MODEL_APP_STATUS,
            Self::Unbind => CONFIG_MODEL_APP_UNBIND,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum ModelPublication {
    Get,
    Status,
    VirtualAddressSet,
}

#[allow(unused)]
impl ModelPublication {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_MODEL_PUBLICATION_GET,
            Self::Status => CONFIG_MODEL_PUBLICATION_STATUS,
            Self::VirtualAddressSet => CONFIG_MODEL_PUBLICATION_VIRTUAL_ADDRESS_SET,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum ModelSubscription {
    Add,
    Delete,
    DeleteAll,
    Overwrite,
    Status,
    VirtualAddressAdd,
    VirtualAddressDelete,
    VirtualAddressOverwrite,
}

#[allow(unused)]
impl ModelSubscription {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Add => CONFIG_MODEL_SUBSCRIPTION_ADD,
            Self::Delete => CONFIG_MODEL_SUBSCRIPTION_DELETE,
            Self::DeleteAll => CONFIG_MODEL_SUBSCRIPTION_DELETE_ALL,
            Self::Overwrite => CONFIG_MODEL_SUBSCRIPTION_OVERWRITE,
            Self::Status => CONFIG_MODEL_SUBSCRIPTION_STATUS,
            Self::VirtualAddressAdd => CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_ADD,
            Self::VirtualAddressDelete => CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_DELETE,
            Self::VirtualAddressOverwrite => CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_OVERWRITE,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum NetKey {
    Add,
    Delete,
    Get,
    List,
    Status,
    Update,
}

#[allow(unused)]
impl NetKey {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Add => CONFIG_NETKEY_ADD,
            Self::Delete => CONFIG_NETKEY_DELETE,
            Self::Get => CONFIG_NETKEY_GET,
            Self::List => CONFIG_NETKEY_LIST,
            Self::Status => CONFIG_NETKEY_STATUS,
            Self::Update => CONFIG_NETKEY_UPDATE,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum NetworkTransmit {
    Get,
    Set,
    Status,
}

#[allow(unused)]
impl NetworkTransmit {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_NETWORK_TRANSMIT_GET,
            Self::Set => CONFIG_NETWORK_TRANSMIT_SET,
            Self::Status => CONFIG_NETWORK_TRANSMIT_STATUS,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum NodeIdentity {
    Get,
    Set,
    Status,
}

#[allow(unused)]
impl NodeIdentity {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_NODE_IDENTITY_GET,
            Self::Set => CONFIG_NODE_IDENTITY_SET,
            Self::Status => CONFIG_NODE_IDENTITY_STATUS,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum Relay {
    Get,
    Set,
    Status,
}

#[allow(unused)]
impl Relay {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_RELAY_GET,
            Self::Set => CONFIG_RELAY_SET,
            Self::Status => CONFIG_RELAY_STATUS,
        }
    }

    pub fn parse_get(parameters: &[u8]) -> Result<Self, ParseError> {
        if parameters.is_empty() {
            Ok(Self::Get)
        } else {
            Err(ParseError::InvalidLength)
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum SIGModel {
    App(SIGModelApp),
    Subscription(SIGModelSubscription),
}

#[allow(unused)]
impl SIGModel {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::App(inner) => inner.opcode(),
            Self::Subscription(inner) => inner.opcode(),
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum SIGModelApp {
    Get,
    List,
}

#[allow(unused)]
impl SIGModelApp {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_SIG_MODEL_APP_GET,
            Self::List => CONFIG_SIG_MODEL_APP_LIST,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum SIGModelSubscription {
    Get,
    List,
}

#[allow(unused)]
impl SIGModelSubscription {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_SIG_MODEL_SUBSCRIPTION_GET,
            Self::List => CONFIG_SIG_MODEL_SUBSCRIPTION_LIST,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum VendorModel {
    App(VendorModelApp),
    Susbcription(VendorModelSubscription),
}

#[allow(unused)]
impl VendorModel {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::App(inner) => inner.opcode(),
            Self::Susbcription(inner) => inner.opcode(),
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum VendorModelApp {
    Get,
    List,
}

#[allow(unused)]
impl VendorModelApp {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_VENDOR_MODEL_APP_GET,
            Self::List => CONFIG_VENDOR_MODEL_APP_LIST,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum VendorModelSubscription {
    Get,
    List,
}

#[allow(unused)]
impl VendorModelSubscription {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => CONFIG_VENDOR_MODEL_SUBSCRIPTION_GET,
            Self::List => CONFIG_VENDOR_MODEL_SUBSCRIPTION_LIST,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum Health {
    Attention(Attention),
    CurrentStatus,
    Fault(Fault),
    Period(Period),
}

#[allow(unused)]
impl Health {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::CurrentStatus => HEALTH_CURRENT_STATUS,
            Self::Attention(inner) => inner.opcode(),
            Self::Fault(inner) => inner.opcode(),
            Self::Period(inner) => inner.opcode(),
        }
    }

    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum Attention {
    Get,
    Set,
    SetUnacknowledged,
}

#[allow(unused)]
impl Attention {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => HEALTH_ATTENTION_GET,
            Self::Set => HEALTH_ATTENTION_SET,
            Self::SetUnacknowledged => HEALTH_ATTENTION_SET_UNACKNOWLEDGED,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum Fault {
    Clear,
    ClearUnacknowledged,
    Get,
    Status,
    Test,
    TestUnacknowledged,
}

#[allow(unused)]
impl Fault {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Clear => HEALTH_FAULT_CLEAR,
            Self::ClearUnacknowledged => HEALTH_FAULT_CLEAR_UNACKNOWLEDGED,
            Self::Get => HEALTH_FAULT_GET,
            Self::Status => HEALTH_FAULT_STATUS,
            Self::Test => HEALTH_FAULT_TEST,
            Self::TestUnacknowledged => HEALTH_FAULT_TEST_UNACKNOWLEDGED,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Format)]
pub enum Period {
    Get,
    Set,
    SetUnacknowledged,
    Status,
}

#[allow(unused)]
impl Period {
    pub fn opcode(&self) -> Opcode {
        match self {
            Self::Get => HEALTH_PERIOD_GET,
            Self::Set => HEALTH_PERIOD_SET,
            Self::SetUnacknowledged => HEALTH_PERIOD_SET_UNACKNOWLEDGED,
            Self::Status => HEALTH_PERIOD_STATUS,
        }
    }
    pub fn emit<const N: usize>(&self, xmit: &mut Vec<u8, N>) -> Result<(), InsufficientBuffer> {
        todo!()
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Opcode {
    OneOctet(u8),
    TwoOctet(u8, u8),
    ThreeOctet(u8, u8, u8),
}

#[allow(unused)]
impl Format for Opcode {
    fn format(&self, fmt: Formatter) {
        match self {
            Opcode::OneOctet(a) => {
                defmt::write!(fmt, "{:x}", a)
            }
            Opcode::TwoOctet(a, b) => {
                defmt::write!(fmt, "{:x} {:x}", a, b)
            }
            Opcode::ThreeOctet(a, b, c) => {
                defmt::write!(fmt, "{:x} {:x} {:x}", a, b, c)
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
//opcode!( CONFIG_COMPOSITION_DATA_GET 0x80, 0x08 );
//opcode!( CONFIG_COMPOSITION_DATA_STATUS 0x02 );
opcode!( CONFIG_CONFIG_MODEL_PUBLICATION_SET 0x03 );
//opcode!( CONFIG_DEFAULT_TTL_GET 0x80, 0x0C );
//opcode!( CONFIG_DEFAULT_TTL_SET 0x80, 0x0D );
//opcode!( CONFIG_DEFAULT_TTL_STATUS 0x80, 0x0E );
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
opcode!( CONFIG_MODEL_APP_BIND 0x80, 0x3D);
opcode!( CONFIG_MODEL_APP_STATUS 0x80, 0x3E);
opcode!( CONFIG_MODEL_APP_UNBIND 0x80, 0x3F);
opcode!( CONFIG_MODEL_PUBLICATION_GET 0x80, 0x18);
opcode!( CONFIG_MODEL_PUBLICATION_STATUS 0x80, 0x19);
opcode!( CONFIG_MODEL_PUBLICATION_VIRTUAL_ADDRESS_SET 0x80, 0x1A);
opcode!( CONFIG_MODEL_SUBSCRIPTION_ADD 0x80, 0x1B);
opcode!( CONFIG_MODEL_SUBSCRIPTION_DELETE 0x80, 0x1C);
opcode!( CONFIG_MODEL_SUBSCRIPTION_DELETE_ALL 0x80, 0x1D);
opcode!( CONFIG_MODEL_SUBSCRIPTION_OVERWRITE 0x80, 0x1E);
opcode!( CONFIG_MODEL_SUBSCRIPTION_STATUS 0x80, 0x1F);
opcode!( CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_ADD 0x80, 0x20);
opcode!( CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_DELETE 0x80, 0x21);
opcode!( CONFIG_MODEL_SUBSCRIPTION_VIRTUAL_ADDRESS_OVERWRITE 0x80, 0x22);
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
