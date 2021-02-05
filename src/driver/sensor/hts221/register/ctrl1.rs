use crate::hal::i2c::I2cAddress;
use crate::prelude::Address;
use embedded_hal::blocking::i2c::{Write, WriteRead};
use crate::driver::sensor::hts221::register::ModifyError;
use crate::driver::i2c::I2cPeripheral;

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
    pub async fn read<I: WriteRead>(
        address: I2cAddress,
        i2c: Address<I2cPeripheral<I>>,
    ) -> Result<Ctrl1, I::Error> {
        unsafe {
            // # Safety
            // The call to `.write_read` is properly awaited for completion before allowing the buffer to drop.
            let mut buf = [0; 1];
            let result = i2c
                .write_read(address, &[CTRL_REG1], &mut buf)
                .await?;
            Ok(buf[0].into())
        }
    }

    pub async fn write<I: Write>(
        address: I2cAddress,
        i2c: Address<I2cPeripheral<I>>,
        reg: Ctrl1,
    ) -> Result<(), I::Error> {
        unsafe {
            // # Safety
            // The call to `.write` is properly awaited for completion before allowing the buffer to drop.
            let bytes = [CTRL_REG1, reg.into()];
            let result = i2c.write(address, &bytes).await?;
        }
        Ok(())
    }

    pub async fn modify<I: WriteRead + Write, F: FnOnce(&mut Ctrl1)>(
        address: I2cAddress,
        i2c: Address<I2cPeripheral<I>>,
        modify: F,
    ) -> Result<(),ModifyError< <I as WriteRead>::Error, <I as Write>::Error>> {
        let mut reg = Self::read(address, i2c).await.map_err( ModifyError::Read)?;
        modify(&mut reg);
        Self::write(address, i2c, reg).await.map_err(ModifyError::Write)
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
