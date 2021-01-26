pub mod who_am_i;
pub mod status;
pub mod calibration;
pub mod t_out;
pub mod h_out;
pub mod ctrl1;


pub const CTRL_REG2: u8 = 0x21;
pub const CTRL_REG3: u8 = 0x22;

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

