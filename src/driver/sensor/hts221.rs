use crate::bind::Bind;
use crate::prelude::*;
use crate::synchronization::Mutex;
use core::fmt::Debug;
use core::ops::Add;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use embedded_hal::digital::v2::InputPin;
use crate::hal::gpio::exti_pin::ExtiPin;
use cortex_m::interrupt::Nr;

const ADDR: u8 = 0x5F;
const WRITE: u8 = 0xBE;
const READ: u8 = 0xBF;

const WHO_AM_I: u8 = 0x0F;

const CTRL_REG1: u8 = 0x20;
const CTRL_REG2: u8 = 0x21;
const CTRL_REG3: u8 = 0x22;

const STATUS: u8 = 0x27;

const H_OUT_L: u8 = 0x28;
const H_OUT_H: u8 = 0x29;
// auto-increment variant
const H_OUT: u8 = 0xA8;

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

pub struct Hts221<P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    sensor: ActorContext<Sensor<I>>,
    ready: InterruptContext<Ready<P, I>>,
    sensor_addr: Option<Address<Sensor<I>>>,
}

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write> Hts221<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    pub fn new<N: Nr>(ready: P, irq: N) -> Self {
        Self {
            sensor: ActorContext::new(Sensor::new()),
            ready: InterruptContext::new(Ready::new(ready), irq),
            sensor_addr: None,
        }
    }

    pub fn start(&'static mut self, supervisor: &mut Supervisor) -> Address<Sensor<I>> {
        let ready_addr = self.ready.start(supervisor);
        let sensor_addr = self.sensor.start(supervisor);
        ready_addr.bind(&sensor_addr);
        self.sensor_addr.replace(sensor_addr.clone());
        sensor_addr
    }
}

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write> Bind<Mutex<I>> for Hts221<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    fn on_bind(&'static mut self, address: Address<Mutex<I>>) {
        //self.i2c.replace(address);
        self.sensor_addr.as_ref().unwrap().bind(&address);
    }
}


pub struct Sensor<I: WriteRead + Read + Write>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    i2c: Option<Address<Mutex<I>>>,
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
    power_down: Power,
    block_data_update: BlockDataUpdate,
    output_data_rate: OutputDataRate,
}

#[derive(Debug, Copy, Clone)]
pub struct CtrlReg2 {
    boot: bool,
    heater: bool,
    one_shot: bool,
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
    active: ActiveState,
    mode: ReadyMode,
    enable: bool,
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

impl<I: WriteRead + Read + Write> Sensor<I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    pub fn new() -> Self {
        Self { i2c: None }
    }

    fn read_who_am_i(i2c: &mut I) -> u8 {
        let mut buf = [0; 1];
        let result = i2c.write_read(ADDR, &[WHO_AM_I], &mut buf);
        log::trace!("[read_who_am_i] result {:?} {:x}", result, buf[0]);
        buf[0]
    }

    fn read_status(i2c: &mut I) -> u8 {
        let mut buf = [0; 1];
        let result = i2c.write_read(ADDR, &[STATUS], &mut buf);
        log::trace!("[read_status] result {:?} {:b}", result, buf[0]);
        buf[0]
    }

    // ------------------------------------------------------------------------
    // CTRL_REG1
    // ------------------------------------------------------------------------

    fn read_ctrl_reg1(i2c: &mut I) -> CtrlReg1 {
        let mut buf = [0; 1];
        let result = i2c.write_read(ADDR, &[CTRL_REG1], &mut buf);
        log::trace!("[read_ctrl_reg1] result {:?} {}", result, buf[0]);
        let reg = CtrlReg1::from(buf[0]);
        log::trace!("[read_ctrl_reg1] reg {:?}", reg);
        reg
    }

    fn write_ctrl_reg1(i2c: &mut I, reg: CtrlReg1) {
        log::trace!("[write_ctrl_reg1] {:?} {}", reg, u8::from(reg));
        let result = i2c.write(ADDR, &[CTRL_REG1, reg.into()]);
        log::trace!("[write_ctrl_reg1] result {:?}", result);
    }

    fn modify_ctrl_reg1<F: FnOnce(CtrlReg1) -> CtrlReg1>(i2c: &mut I, modify: F) {
        let reg = Self::read_ctrl_reg1(i2c);
        let reg = modify(reg);
        Self::write_ctrl_reg1(i2c, reg)
    }

    // ------------------------------------------------------------------------
    // CTRL_REG2
    // ------------------------------------------------------------------------

    fn read_ctrl_reg2(i2c: &mut I) -> CtrlReg2 {
        let mut buf = [0; 1];
        let result = i2c.write_read(ADDR, &[CTRL_REG2], &mut buf);
        log::trace!("[read_ctrl_reg2] result {:?} {}", result, buf[0]);
        let reg = CtrlReg2::from(buf[0]);
        log::trace!("[read_ctrl_reg2] reg {:?}", reg);
        reg
    }

    fn write_ctrl_reg2(i2c: &mut I, reg: CtrlReg2) {
        log::trace!("[write_ctrl_reg2] {:?} {}", reg, u8::from(reg));
        let result = i2c.write(ADDR, &[CTRL_REG2, reg.into()]);
        log::trace!("[write_ctrl_reg2] result {:?}", result);
    }

    fn modify_ctrl_reg2<F: FnOnce(CtrlReg2) -> CtrlReg2>(i2c: &mut I, modify: F) {
        let reg = Self::read_ctrl_reg2(i2c);
        let reg = modify(reg);
        Self::write_ctrl_reg2(i2c, reg)
    }

    // ------------------------------------------------------------------------
    // CTRL_REG3
    // ------------------------------------------------------------------------

    fn read_ctrl_reg3(i2c: &mut I) -> CtrlReg3 {
        let mut buf = [0; 1];
        let result = i2c.write_read(ADDR, &[CTRL_REG3], &mut buf);
        log::trace!("[read_ctrl_reg3] result {:?} {}", result, buf[0]);
        let reg = CtrlReg3::from(buf[0]);
        log::trace!("[read_ctrl_reg3] reg {:?} {:b}", reg, buf[0]);
        reg
    }

    fn write_ctrl_reg3(i2c: &mut I, reg: CtrlReg3) {
        log::trace!("[write_ctrl_reg3] {:?} {}", reg, u8::from(reg));
        let result = i2c.write(ADDR, &[CTRL_REG3, reg.into()]);
        log::trace!("[write_ctrl_reg3] result {:?}", result);
    }

    fn modify_ctrl_reg3<F: FnOnce(CtrlReg3) -> CtrlReg3>(i2c: &mut I, modify: F) {
        let reg = Self::read_ctrl_reg3(i2c);
        let reg = modify(reg);
        Self::write_ctrl_reg3(i2c, reg)
    }

    // ------------------------------------------------------------------------
    // Humidify
    // ------------------------------------------------------------------------

    fn read_h_out(i2c: &mut I) -> i16 {
        let mut buf = [0; 2];
        let result = i2c.write_read(ADDR, &[H_OUT], &mut buf);
        log::trace!("[read_h_out] result {:?} - {:?}", result, buf);
        i16::from_le_bytes(buf)
    }

    // ------------------------------------------------------------------------
    // Temperature
    // ------------------------------------------------------------------------

    fn read_t_out(i2c: &mut I) -> i16 {
        let mut buf = [0; 2];
        let result = i2c.write_read(ADDR, &[T_OUT], &mut buf);
        log::trace!("[read_t_out] result {:?} - {:?}", result, buf);
        i16::from_le_bytes(buf)
    }

    fn read_t0_out(i2c: &mut I) -> i16 {
        let mut buf = [0; 2];
        let result = i2c.write_read(ADDR, &[T0_OUT], &mut buf);
        log::trace!("[read_t0_out] result {:?}", result);
        i16::from_le_bytes(buf)
    }

    fn read_t0_degc_x8(i2c: &mut I) -> u8 {
        let mut buf = [0; 1];
        let result = i2c.write_read(ADDR, &[T0_DEGC_X8], &mut buf);
        log::trace!("[read_t0_degc] result {:?}", result);
        buf[0]
    }

    fn read_t1_out(i2c: &mut I) -> i16 {
        let mut buf = [0; 2];
        let result = i2c.write_read(ADDR, &[T1_OUT], &mut buf);
        log::trace!("[read_t1_out] result {:?}", result);
        i16::from_le_bytes(buf)
    }

    fn read_t1_degc_x8(i2c: &mut I) -> u8 {
        let mut buf = [0; 1];
        let result = i2c.write_read(ADDR, &[T1_DEGC_X8], &mut buf);
        log::trace!("[read_t1_degc_x8] result {:?}", result);
        buf[0]
    }

    fn read_t1_t0_msb(i2c: &mut I) -> (u8, u8) {
        let mut buf = [0; 1];
        let result = i2c.write_read(ADDR, &[T1_T0_MSB], &mut buf);
        log::trace!("[read_t1_t0_msb] result {:?}", result);

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

impl<I: WriteRead + Read + Write> Actor for Sensor<I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    type Event = ();
}

impl<I: WriteRead + Read + Write> Bind<Mutex<I>> for Sensor<I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    fn on_bind(&'static mut self, address: Address<Mutex<I>>) {
        self.i2c.replace(address);
    }
}

pub struct Initialize;

impl<I: WriteRead + Read + Write> NotificationHandler<Initialize> for Sensor<I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    fn on_notification(&'static mut self, message: Initialize) -> Completion {
        log::trace!(" ------- initialize sensor");
        Completion::defer(async move {
            if let Some(ref i2c) = self.i2c {
                let mut i2c = i2c.lock().await;

                Self::modify_ctrl_reg2(&mut i2c, |mut reg| {
                    reg.boot = false;
                    reg.one_shot = true;
                    reg
                });

                Self::modify_ctrl_reg1(&mut i2c, |mut reg| {
                    reg.power_down = Power::Active;
                    reg.output_data_rate = OutputDataRate::Hz7;
                    reg.block_data_update = BlockDataUpdate::MsbLsbReading;
                    reg
                });

                Self::modify_ctrl_reg3(&mut i2c, |mut reg| {
                    reg.enable = true;
                    //reg.active = ActiveState::High;
                    //reg.mode = ReadyMode::PushPull;
                    reg
                });

                Self::read_who_am_i(&mut i2c);
                Self::read_status(&mut i2c);
                loop {
                    if Self::read_status(&mut i2c) == 0 {
                        break;
                    }
                    Self::read_h_out(&mut i2c);
                    Self::read_t_out(&mut i2c);
                }
            }
        })
        //Completion::immediate()
    }
}

pub struct TakeReading;

impl<I: WriteRead + Read + Write> NotificationHandler<TakeReading> for Sensor<I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    fn on_notification(&'static mut self, message: TakeReading) -> Completion {
        Completion::defer(async move {
            if let Some(ref i2c) = self.i2c {
                let mut i2c = i2c.lock().await;
                let h = Self::read_h_out(&mut i2c);
                let temp_degc = Self::calibrated_temperature_degc(&mut i2c);
                log::info!("[hts221] temperature is {} °F", Self::c_to_f(temp_degc));
                Self::modify_ctrl_reg3(&mut i2c, |mut reg| {
                    reg.enable = true;
                    reg
                });
            }
        })
    }
}

impl<I: WriteRead + Read + Write + 'static> Address<Sensor<I>>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    pub fn trigger_read_temperature(&self) {
        self.notify(TakeReading)
    }
}


pub struct Ready<P: InputPin + ExtiPin, I: WriteRead + Read + Write>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    pin: P,
    sensor: Option<Address<Sensor<I>>>,
}

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write> Ready<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    pub fn new(pin: P) -> Self {
        Self {
            pin,
            sensor: None,
        }
    }
}

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write> Actor for Ready<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    type Event = ();
}

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write + 'static> Interrupt for Ready<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    fn on_interrupt(&mut self) {
        if self.pin.check_interrupt() {
            log::trace!("[hts221] READY");
            if let Some(sensor) = self.sensor.as_ref() {
                sensor.trigger_read_temperature()
            }
            self.pin.clear_interrupt_pending_bit();
        }
    }
}

impl<P: InputPin + ExtiPin, I: WriteRead + Read + Write> Bind<Sensor<I>> for Ready<P, I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    fn on_bind(&'static mut self, address: Address<Sensor<I>>) {
        self.sensor.replace(address);
    }
}


impl<I: WriteRead + Read + Write + 'static> Address<Sensor<I>>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    pub fn initialize(&self) {
        self.notify(Initialize);
    }
}