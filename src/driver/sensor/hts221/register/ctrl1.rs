use crate::hal::i2c::I2cAddress;
use core::cell::Ref;
use core::ops::DerefMut;
use embedded_hal::blocking::i2c::{Write, WriteRead};

const CTRL_REG1: u8 = 0x20;

#[derive(Debug, Copy, Clone)]
pub enum Power {
    PowerDown,
    Active,
}

#[derive(Debug, Copy, Clone)]
pub enum BlockDataUpdate {
    Continuous,
    MsbLsbReading,
}

#[derive(Debug, Copy, Clone)]
pub enum OutputDataRate {
    OneShot,
    Hz1,
    Hz7,
    Hz12p5,
}

#[derive(Debug, Copy, Clone)]
pub struct Ctrl1 {
    power_down: Power,
    block_data_update: BlockDataUpdate,
    output_data_rate: OutputDataRate,
}

impl Ctrl1 {
    pub fn read<I: DerefMut<Target = I2C>, I2C: WriteRead>(
        address: I2cAddress,
        i2c: &mut I,
    ) -> Self {
        let mut buf = [0; 1];
        let result = i2c.write_read(address.into(), &[CTRL_REG1], &mut buf);
        buf[0].into()
    }

    pub fn write<I: DerefMut<Target = I2C>, I2C: Write>(
        address: I2cAddress,
        i2c: &mut I,
        reg: Ctrl1,
    ) {
        let result = i2c.write(address.into(), &[CTRL_REG1, reg.into()]);
    }

    pub fn modify<I: DerefMut<Target = I2C>, I2C: WriteRead + Write, F: FnOnce(&mut Ctrl1)>(
        address: I2cAddress,
        i2c: &mut I,
        modify: F,
    ) {
        let mut reg = Self::read(address, i2c);
        modify(&mut reg);
        Self::write(address, i2c, reg);
    }

    pub fn power_down(&mut self) -> &Self {
        self.power_down = Power::PowerDown;
        self
    }

    pub fn power_active(&mut self) -> &mut Self {
        self.power_down = Power::Active;
        self
    }

    pub fn output_data_rate(&mut self, odr: OutputDataRate) -> &mut Self {
        self.output_data_rate = odr;
        self
    }

    pub fn block_data_update(&mut self, bdu: BlockDataUpdate) -> &mut Self {
        self.block_data_update = bdu;
        self
    }
}

impl Into<Power> for u8 {
    fn into(self) -> Power {
        if (self & 0x80) != 0 {
            Power::Active
        } else {
            Power::PowerDown
        }
    }
}

impl From<Power> for u8 {
    fn from(p: Power) -> Self {
        match p {
            Power::PowerDown => 0b00000000,
            Power::Active => 0b10000000,
        }
    }
}

impl Into<BlockDataUpdate> for u8 {
    fn into(self) -> BlockDataUpdate {
        if (self & 0x40) != 0 {
            BlockDataUpdate::MsbLsbReading
        } else {
            BlockDataUpdate::Continuous
        }
    }
}

impl From<BlockDataUpdate> for u8 {
    fn from(bdu: BlockDataUpdate) -> u8 {
        match bdu {
            BlockDataUpdate::Continuous => 0b000,
            BlockDataUpdate::MsbLsbReading => 0b100,
        }
    }
}

impl Into<OutputDataRate> for u8 {
    fn into(self) -> OutputDataRate {
        let v = self & 0b11;

        match v {
            0b01 => OutputDataRate::Hz1,
            0b10 => OutputDataRate::Hz7,
            0b11 => OutputDataRate::Hz12p5,
            _ => OutputDataRate::OneShot,
        }
    }
}

impl From<OutputDataRate> for u8 {
    fn from(odr: OutputDataRate) -> Self {
        match odr {
            OutputDataRate::OneShot => 0b00,
            OutputDataRate::Hz1 => 0b01,
            OutputDataRate::Hz7 => 0b10,
            OutputDataRate::Hz12p5 => 0b11,
        }
    }
}

impl Into<Ctrl1> for u8 {
    fn into(self) -> Ctrl1 {
        Ctrl1 {
            power_down: self.into(),
            output_data_rate: self.into(),
            block_data_update: self.into(),
        }
    }
}

impl Into<u8> for Ctrl1 {
    fn into(self) -> u8 {
        u8::from(self.power_down)
            | u8::from(self.output_data_rate)
            | u8::from(self.block_data_update)
    }
}
