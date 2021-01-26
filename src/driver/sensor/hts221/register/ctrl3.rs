use embedded_hal::blocking::i2c::{WriteRead, Write};
use crate::hal::i2c::I2cAddress;
use core::cell::Ref;
use core::ops::DerefMut;

const CTRL_REG3: u8 = 0x22;

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

pub struct Ctrl3 {
    pub active: ActiveState,
    pub mode: ReadyMode,
    pub enable: bool,
}

impl Ctrl3 {
    pub fn read<I: DerefMut<Target=I2C>, I2C: WriteRead>(address: I2cAddress, i2c: &mut I) -> Ctrl3 {
        let mut buf = [0;1];
        let result = i2c.write_read( address.into(), &[CTRL_REG3], &mut buf);
        buf[0].into()
    }

    pub fn write<I: DerefMut<Target=I2C>, I2C: Write>(address: I2cAddress, i2c: &mut I, reg: Ctrl3) {
        let result = i2c.write(address.into(), &[CTRL_REG3, reg.into()] );
    }

    pub fn modify<I: DerefMut<Target=I2C>, I2C: WriteRead + Write, F: FnOnce(&mut Ctrl3)>(address: I2cAddress, i2c: &mut I, modify: F)  {
        let mut reg = Self::read(address, i2c);
        modify(&mut reg);
        Self::write(address, i2c, reg);
    }

    pub fn active_state(&mut self, active_state: ActiveState) -> &mut Self {
        self.active = active_state;
        self
    }

    pub fn enable(&mut self, enable: bool) -> &mut Self {
        self.enable = enable;
        self
    }

    pub fn ready_mode(&mut self, ready_mode: ReadyMode) -> &mut Self {
        self.mode = ready_mode;
        self
    }
}

impl Into<ReadyMode> for u8 {
    fn into(self) -> ReadyMode {
        if (self & 0b01000000 ) != 0 {
            ReadyMode::OpenDrain
        } else {
            ReadyMode::PushPull
        }
    }
}

impl From<ReadyMode> for u8 {
    fn from(ready_mode: ReadyMode) -> Self {
        match ready_mode {
            ReadyMode::PushPull => 0b00000000,
            ReadyMode::OpenDrain => 0b01000000
        }
    }
}

impl Into<ActiveState> for u8 {
    fn into(self) -> ActiveState {
        if (self & 0b10000000) != 0 {
            ActiveState::Low
        } else {
            ActiveState::High
        }
    }
}

impl From<ActiveState> for u8 {
    fn from(active_state: ActiveState) -> Self {
        match active_state {
            ActiveState::High => 0b00000000,
            ActiveState::Low => 0b10000000,
        }
    }
}

impl Into<Ctrl3> for u8 {
    fn into(self) -> Ctrl3 {
        Ctrl3 {
            active: self.into(),
            mode: self.into(),
            enable: (self & 0b00000100) != 0,

        }
    }
}

impl Into<u8> for Ctrl3 {
    fn into(self) -> u8 {
        u8::from( self.active )
        | u8::from( self.mode )
        | if self.enable { 0b100 } else {0b000}
    }
}

