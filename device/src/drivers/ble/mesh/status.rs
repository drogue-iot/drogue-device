pub enum Status {
    Success,
    InvalidAddress,
    InvalidModel,
    InvalidAppKeyIndex,
    InvalidNetKeyIndex,
    InsufficientResources,
    KeyIndexAlreadyStored,
    InvalidPublishParameters,
    NotASubscribeModel,
    StorageFailure,
    FeatureNotSupported,
    CannotUpdate,
    CannotRemove,
    CannotBind,
    TemporarilyUnableToChangeState,
    CannotSet,
    UnspecifiedError,
    InvalidBinding,
    RFU(u8),
}

impl From<u8> for Status {
    fn from(val: u8) -> Self {
        match val {
            0x00 => Self::Success,
            0x01 => Self::InvalidAddress,
            0x02 => Self::InvalidModel,
            0x03 => Self::InvalidAppKeyIndex,
            0x04 => Self::InvalidNetKeyIndex,
            0x05 => Self::InsufficientResources,
            0x06 => Self::KeyIndexAlreadyStored,
            0x07 => Self::InvalidPublishParameters,
            0x08 => Self::NotASubscribeModel,
            0x09 => Self::StorageFailure,
            0x0A => Self::FeatureNotSupported,
            0x0B => Self::CannotUpdate,
            0x0C => Self::CannotRemove,
            0x0D => Self::CannotBind,
            0x0E => Self::TemporarilyUnableToChangeState,
            0x0F => Self::CannotSet,
            0x10 => Self::UnspecifiedError,
            0x11 => Self::InvalidBinding,
            _ => Self::RFU(val),
        }
    }
}

impl From<Status> for u8 {
    fn from(val: Status) -> Self {
        match val {
            Status::Success => 0x00,
            Status::InvalidAddress => 0x01,
            Status::InvalidModel => 0x02,
            Status::InvalidAppKeyIndex => 0x03,
            Status::InvalidNetKeyIndex => 0x04,
            Status::InsufficientResources => 0x05,
            Status::KeyIndexAlreadyStored => 0x06,
            Status::InvalidPublishParameters => 0x07,
            Status::NotASubscribeModel => 0x08,
            Status::StorageFailure => 0x09,
            Status::FeatureNotSupported => 0x0A,
            Status::CannotUpdate => 0x0B,
            Status::CannotRemove => 0x0C,
            Status::CannotBind => 0x0D,
            Status::TemporarilyUnableToChangeState => 0x0E,
            Status::CannotSet => 0x0F,
            Status::UnspecifiedError => 0x10,
            Status::InvalidBinding => 0x11,
            Status::RFU(num) => num,
        }
    }
}
