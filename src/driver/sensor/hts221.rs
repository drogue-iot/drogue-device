use crate::prelude::*;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use crate::synchronization::Mutex;
use core::fmt::{Debug};
use crate::bind::Bind;
use core::ops::Add;

const ADDR: u8 = 0x5F;
const WRITE: u8 = 0xBE;
const READ: u8 = 0xBF;

const WHO_AM_I: u8 = 0x0F;

const CTRL_REG1: u8 = 0x20;
const CTRL_REG2: u8 = 0x21;
const CTRL_REG3: u8 = 0x22;

const T_OUT_L: u8 = 0x2A;
const T_OUT_H: u8 = 0x2B;
// auto-increment variant
const T_OUT: u8 = 0xAA;

const T0_OUT_L: u8 = 0x3C;
const T0_OUT_H: u8 = 0x3D;
// auto-increment variant
const T0_OUT: u8 = 0xBC;

const T0_DEGC_X8: u8 = 0x32;

const T1_OUT_L: u8 = 0x3E;
const T1_OUT_H: u8 = 0x3F;
// auto-increment variant
const T1_OUT: u8 = 0xBE;

const T1_DEGC_X8: u8 = 0x33;

const T1_T0_MSB: u8 = 0x35;

pub struct Hts221<I: WriteRead + Read + Write>
    where <I as WriteRead>::Error: Debug,
          <I as Write>::Error: Debug
{
    i2c: Option<Address<Mutex<I>>>
}

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
            BlockDataUpdate::Continuous => {
                0
            }
            BlockDataUpdate::MsbLsbReading => {
                0b00000100
            }
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
            3 =>Self::Hz12p5,
            _ => Self::OneShot
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
pub struct CtrlReg1 {
    power_down: bool,
    block_data_update: BlockDataUpdate,
    output_data_rate: OutputDataRate,
}

#[derive(Debug, Copy, Clone)]
pub struct CtrlReg2 {
    boot: bool,
    heater: bool,
    one_shot: bool,
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
        let power_down        = val & 0b10000000;
        let block_data_update = val & 0b00000010;
        let output_data_rate  = val & 0b00000011;
        Self {
            power_down: power_down != 0,
            block_data_update: BlockDataUpdate::from(block_data_update != 0),
            output_data_rate: OutputDataRate::from(output_data_rate),
        }
    }
}

impl From<CtrlReg1> for u8 {
    fn from(reg: CtrlReg1) -> Self {
        let mut val = 0;
        if reg.power_down {
           val |= 0b10000000;
        }
        val = val | u8::from(reg.block_data_update);
        val = val | u8::from(reg.output_data_rate);
        val
    }
}


impl<I: WriteRead + Read + Write> Hts221<I>
    where <I as WriteRead>::Error: Debug,
          <I as Write>::Error: Debug
{
    pub fn new() -> Self {
        Self {
            i2c: None
        }
    }

    fn read_who_am_i(i2c: &mut I) -> u8 {
        let mut buf = [0; 1];
        i2c.write_read(ADDR, &[WHO_AM_I], &mut buf);
        buf[0]
    }

    // ------------------------------------------------------------------------
    // CTRL_REG1
    // ------------------------------------------------------------------------

    fn read_ctrl_reg1(i2c: &mut I) -> CtrlReg1 {
        let mut buf = [0;1];
        i2c.write_read( ADDR, &[CTRL_REG1], &mut buf);
        CtrlReg1::from(buf[0])
    }

    fn write_ctrl_reg1(i2c: &mut I, reg: CtrlReg1) {
        i2c.write( ADDR, &[CTRL_REG1, reg.into() ]);
    }

    fn modify_ctrl_reg1<F: FnOnce(CtrlReg1)->CtrlReg1>(i2c: &mut I, modify: F) {
        let reg = Self::read_ctrl_reg1(i2c);
        let reg = modify(reg);
        Self::write_ctrl_reg1(i2c, reg)
    }

    // ------------------------------------------------------------------------
    // CTRL_REG2
    // ------------------------------------------------------------------------

    fn read_ctrl_reg2(i2c: &mut I) -> CtrlReg2 {
        let mut buf = [0;1];
        i2c.write_read( ADDR, &[CTRL_REG2], &mut buf);
        CtrlReg2::from(buf[0])
    }

    fn write_ctrl_reg2(i2c: &mut I, reg: CtrlReg2) {
        i2c.write( ADDR, &[CTRL_REG2, reg.into() ]);
    }

    fn modify_ctrl_reg2<F: FnOnce(CtrlReg2)->CtrlReg2>(i2c: &mut I, modify: F) {
        let reg = Self::read_ctrl_reg2(i2c);
        let reg = modify(reg);
        Self::write_ctrl_reg2(i2c, reg)
    }

    // ------------------------------------------------------------------------
    // Temperature
    // ------------------------------------------------------------------------

    fn read_t_out(i2c: &mut I) -> i16 {
        let mut buf = [0; 2];
        i2c.write_read(ADDR, &[T_OUT], &mut buf);
        i16::from_le_bytes(buf)
    }

    fn read_t0_out(i2c: &mut I) -> i16 {
        let mut buf = [0; 2];
        i2c.write_read(ADDR, &[T0_OUT], &mut buf);
        i16::from_le_bytes(buf)
    }

    fn read_t0_degc_x8(i2c: &mut I) -> u8 {
        let mut buf = [0; 1];
        i2c.write_read(ADDR, &[T0_DEGC_X8], &mut buf);
        buf[0]
    }

    fn read_t1_out(i2c: &mut I) -> i16 {
        let mut buf = [0; 2];
        i2c.write_read(ADDR, &[T1_OUT], &mut buf);
        i16::from_le_bytes(buf)
    }

    fn read_t1_degc_x8(i2c: &mut I) -> u8 {
        let mut buf = [0; 1];
        i2c.write_read(ADDR, &[T1_DEGC_X8], &mut buf);
        buf[0]
    }

    fn read_t1_t0_msb(i2c: &mut I) -> (u8, u8) {
        let mut buf = [0; 1];
        i2c.write_read(ADDR, &[T1_T0_MSB], &mut buf);

        let t0_msb = (buf[0] & 0b00000011) >> 2;
        let t1_msb = (buf[0] & 0b00001100) >> 2;

        (t1_msb, t0_msb)
    }

    fn calibrated_temperature_degc(i2c: &mut I) -> f32 {
        let t_out = Self::read_t_out(i2c);

        let t0_out = Self::read_t0_out(i2c);
        let t1_out = Self::read_t1_out(i2c);

        let t0_degc = Self::read_t0_degc_x8(i2c);
        let t1_degc = Self::read_t1_degc_x8(i2c);

        let (t1_msb, t0_msb) = Self::read_t1_t0_msb(i2c);

        let t0_degc = i16::from_le_bytes([t0_degc, t0_msb]) as f32 / 8.0;
        let t1_degc = i16::from_le_bytes([t1_degc, t1_msb]) as f32 / 8.0;

        let slope = (t1_degc - t0_degc) / (t1_out - t0_out) as f32;

        let t_degc = t0_degc as f32 + (slope * (t_out - t0_out) as f32);

        t_degc
    }

    fn c_to_f(c: f32) -> f32 {
        c * (9.0 / 5.0) + 32.0
    }
}

impl<I: WriteRead + Read + Write> Actor for Hts221<I>
    where <I as WriteRead>::Error: Debug,
          <I as Write>::Error: Debug
{}

impl<I: WriteRead + Read + Write> Bind<Mutex<I>> for Hts221<I>
    where <I as WriteRead>::Error: Debug,
          <I as Write>::Error: Debug
{
    fn on_bind(&'static mut self, address: Address<Mutex<I>>) {
        self.i2c.replace( address );
    }
}

pub struct TakeReading;

impl<I: WriteRead + Read + Write> NotificationHandler<TakeReading> for Hts221<I>
    where <I as WriteRead>::Error: Debug,
          <I as Write>::Error: Debug
{
    fn on_notification(&'static mut self, message: TakeReading) -> Completion {
        Completion::defer(async move {
            if let Some(ref i2c) = self.i2c {
                let mut i2c = i2c.lock().await;
                Self::modify_ctrl_reg1(&mut i2c, |mut reg| {
                    reg.power_down = false;
                    reg
                });
                Self::modify_ctrl_reg2(&mut i2c, |mut reg| {
                    reg.one_shot = true;
                    reg
                });
                let temp_degc = Self::calibrated_temperature_degc(&mut i2c);
                log::info!("[hts221] temperature is {} °F", Self::c_to_f(temp_degc));
            }
        })
    }
}

impl<I: WriteRead + Read + Write + 'static> Address<Hts221<I>>
    where <I as WriteRead>::Error: Debug,
          <I as Write>::Error: Debug
{
    pub fn trigger_read_temperature(&self) {
        self.notify( TakeReading )
    }

}
