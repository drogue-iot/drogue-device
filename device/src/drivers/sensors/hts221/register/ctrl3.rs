use crate::traits::i2c::I2cAddress;
use embedded_hal_async::i2c::*;

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
    pub async fn read<I: I2c>(address: I2cAddress, i2c: &mut I) -> Result<Ctrl3, I::Error> {
        let mut buf = [0; 1];
        let _ = i2c
            .write_read(address.into(), &[CTRL_REG3], &mut buf)
            .await?;
        Ok(buf[0].into())
    }

    pub async fn write<I: I2c>(
        address: I2cAddress,
        i2c: &mut I,
        reg: Ctrl3,
    ) -> Result<(), I::Error> {
        Ok(i2c.write(address.into(), &[CTRL_REG3, reg.into()]).await?)
    }

    pub async fn modify<I: I2c, F: FnOnce(&mut Ctrl3)>(
        address: I2cAddress,
        i2c: &mut I,
        modify: F,
    ) -> Result<(), I::Error> {
        let mut reg = Self::read(address, i2c).await?;
        modify(&mut reg);
        Self::write(address, i2c, reg).await
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
        if (self & 0b01000000) != 0 {
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
            ReadyMode::OpenDrain => 0b01000000,
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
        u8::from(self.active) | u8::from(self.mode) | if self.enable { 0b100 } else { 0b000 }
    }
}
