

pub const ADDR: u8 = 0x5F;
pub const WRITE: u8 = 0xBE;
pub const READ: u8 = 0xBF;

pub const WHO_AM_I: u8 = 0x0F;

pub const CTRL_REG1: u8 = 0x20;
pub const CTRL_REG2: u8 = 0x21;
pub const CTRL_REG3: u8 = 0x22;

pub const STATUS: u8 = 0x27;

pub const H_OUT_L: u8 = 0x28;
pub const H_OUT_H: u8 = 0x29;
// auto-increment variant
pub const H_OUT: u8 = 0xA8;

pub const T_OUT_L: u8 = 0x2A;
pub const T_OUT_H: u8 = 0x2B;
// auto-increment variant
pub const T_OUT: u8 = 0xAA;

pub const T0_OUT_L: u8 = 0x3C;
pub const T0_OUT_H: u8 = 0x3D;
// auto-increment variant
pub const T0_OUT: u8 = 0xBC;

pub const T0_DEGC_X8: u8 = 0x32;

pub const T1_OUT_L: u8 = 0x3E;
pub const T1_OUT_H: u8 = 0x3F;
// auto-increment variant
pub const T1_OUT: u8 = 0xBE;

pub const T1_DEGC_X8: u8 = 0x33;

pub const T1_T0_MSB: u8 = 0x35;

#[derive(Debug, Copy, Clone)]
pub enum BlockDataUpdate {
    Continuous,
    MsbLsbReading,
}

impl From<bool> for BlockDataUpdate {
    fn from(val: bool) -> Self {
        if val {
            Self::MsbLsbReading
        } else {
            Self::Continuous
        }
    }
}

impl From<BlockDataUpdate> for u8 {
    fn from(bdu: BlockDataUpdate) -> Self {
        match bdu {
            BlockDataUpdate::Continuous => 0,
            BlockDataUpdate::MsbLsbReading => 0b00000100,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum OutputDataRate {
    OneShot,
    Hz1,
    Hz7,
    Hz12p5,
}

impl From<u8> for OutputDataRate {
    fn from(val: u8) -> Self {
        match val {
            1 => Self::Hz7,
            2 => Self::Hz7,
            3 => Self::Hz12p5,
            _ => Self::OneShot,
        }
    }
}

impl From<OutputDataRate> for u8 {
    fn from(odr: OutputDataRate) -> Self {
        match odr {
            OutputDataRate::OneShot => 0,
            OutputDataRate::Hz1 => 1,
            OutputDataRate::Hz7 => 2,
            OutputDataRate::Hz12p5 => 3,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Power {
    PowerDown,
    Active,
}

#[derive(Debug, Copy, Clone)]
pub struct CtrlReg1 {
    pub power_down: Power,
    pub block_data_update: BlockDataUpdate,
    pub output_data_rate: OutputDataRate,
}

#[derive(Debug, Copy, Clone)]
pub struct CtrlReg2 {
    pub boot: bool,
    pub heater: bool,
    pub one_shot: bool,
}

#[derive(Debug, Copy, Clone)]
pub enum ReadyMode {
    PushPull,
    OpenDrain,
}

#[derive(Debug, Copy, Clone)]
pub enum ActiveState {
    High,
    Low,
}

#[derive(Debug, Copy, Clone)]
pub struct CtrlReg3 {
    pub active: ActiveState,
    pub mode: ReadyMode,
    pub enable: bool,
}

impl From<u8> for CtrlReg3 {
    fn from(val: u8) -> Self {
        let active = val & 0b10000000;
        let mode = val & 0b01000000;
        let enable = val & 0b00000100;

        let active = if active == 0 {
            ActiveState::High
        } else {
            ActiveState::Low
        };

        let mode = if mode == 0 {
            ReadyMode::PushPull
        } else {
            ReadyMode::OpenDrain
        };

        let enable = (enable != 0);

        Self {
            active,
            mode,
            enable,
        }
    }
}

impl From<CtrlReg3> for u8 {
    fn from(reg: CtrlReg3) -> Self {
        let mut val = 0;

        val |= match reg.active {
            ActiveState::High => { 0 }
            ActiveState::Low => { 0b10000000 }
        };

        val |= match reg.mode {
            ReadyMode::PushPull => { 0 }
            ReadyMode::OpenDrain => { 0b01000000 }
        };

        val |= if reg.enable {
            0b100
        } else {
            0
        };

        val
    }
}

impl From<u8> for CtrlReg2 {
    fn from(val: u8) -> Self {
        let boot = val & 0b10000000;
        let heater = val & 0b00000010;
        let one_shot = val & 0b00000001;
        Self {
            boot: boot != 0,
            heater: heater != 0,
            one_shot: one_shot != 0,
        }
    }
}

impl From<CtrlReg2> for u8 {
    fn from(reg: CtrlReg2) -> Self {
        let mut val = 0;
        if reg.boot {
            val |= 0b10000000;
        }
        if reg.heater {
            val |= 0b00000010;
        }
        if reg.one_shot {
            val |= 0b00000001;
        }
        val
    }
}

impl From<u8> for CtrlReg1 {
    fn from(val: u8) -> Self {
        let power_down = val & 0b10000000;
        let block_data_update = val & 0b00000100;
        let output_data_rate = val & 0b00000011;

        let power_down = if power_down == 0 {
            Power::PowerDown
        } else {
            Power::Active
        };

        Self {
            power_down,
            block_data_update: BlockDataUpdate::from(block_data_update != 0),
            output_data_rate: OutputDataRate::from(output_data_rate),
        }
    }
}

impl From<CtrlReg1> for u8 {
    fn from(reg: CtrlReg1) -> Self {
        let mut val = 0;
        val |= match reg.power_down {
            Power::PowerDown => { 0 }
            Power::Active => { 0b10000000 }
        };
        val = val | u8::from(reg.block_data_update);
        val = val | u8::from(reg.output_data_rate);
        val
    }
}