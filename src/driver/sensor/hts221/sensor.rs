use crate::bind::Bind;
use crate::prelude::*;
use crate::synchronization::Mutex;
use core::fmt::Debug;
use core::ops::Add;
use embedded_hal::blocking::i2c::{Read, Write, WriteRead};
use embedded_hal::digital::v2::InputPin;
use crate::hal::gpio::exti_pin::ExtiPin;
use cortex_m::interrupt::Nr;
use crate::driver::sensor::hts221::ready::{Ready, DataReady};
use crate::driver::sensor::hts221::register::*;
use crate::driver::sensor::hts221::calibration::{Calibration, TemperatureCalibration};
use core::default::Default;


pub struct Sensor<I: WriteRead + Read + Write + 'static>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    i2c: Option<Address<Mutex<I>>>,
    calibration: Calibration,
}

impl<I: WriteRead + Read + Write + 'static> Sensor<I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    pub fn new() -> Self {
        Self {
            i2c: None,
            calibration: Calibration::new(),
        }
    }

    // ------------------------------------------------------------------------
    // Lifecycle
    // ------------------------------------------------------------------------

    fn initialize(&'static mut self) -> Completion {
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
                //Self::read_status(&mut i2c);
                loop {
                    if Self::read_status(&mut i2c) == 0 {
                        break;
                    }
                    Self::read_h_out(&mut i2c);
                    Self::read_t_out(&mut i2c);
                }
            }
        })
    }

    fn start(&'static mut self) -> Completion {
        Completion::defer(async move {
            self.load_calibration().await;
        })
    }

    async fn load_calibration(&'static mut self) {
        if let Some(ref i2c) = self.i2c {
            let mut i2c = i2c.lock().await;

            let t0_out = Self::read_t0_out(&mut i2c);
            let t1_out = Self::read_t1_out(&mut i2c);

            let t0_degc = Self::read_t0_degc_x8(&mut i2c);
            let t1_degc = Self::read_t1_degc_x8(&mut i2c);

            let (t1_msb, t0_msb) = Self::read_t1_t0_msb(&mut i2c);

            let t0_degc = i16::from_le_bytes([t0_degc, t0_msb]) as f32 / 8.0;
            let t1_degc = i16::from_le_bytes([t1_degc, t1_msb]) as f32 / 8.0;

            let slope = (t1_degc - t0_degc) / (t1_out - t0_out) as f32;

            self.calibration.temperature.replace(
                TemperatureCalibration {
                    t0_out,
                    t1_out,
                    t0_degc,
                    t1_degc,
                    slope,
                }
            );
        }
    }

    // ------------------------------------------------------------------------
    // WHO_AM_I
    // ------------------------------------------------------------------------

    fn read_who_am_i(i2c: &mut I) -> u8 {
        let mut buf = [0; 1];
        let result = i2c.write_read(ADDR, &[WHO_AM_I], &mut buf);
        log::trace!("[read_who_am_i] result {:?} {:x}", result, buf[0]);
        buf[0]
    }

    // ------------------------------------------------------------------------
    // STATUS
    // ------------------------------------------------------------------------

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

    fn calibrated_temperature_degc(&self, i2c: &mut I) -> f32 {
        if let Some(ref calibration) = self.calibration.temperature {
            let t_out = Self::read_t_out(i2c);
            let calibration = self.calibration.temperature.as_ref().unwrap();
            let t_degc = calibration.t0_degc as f32 + (calibration.slope * (t_out - calibration.t0_out) as f32);
            t_degc
        } else {
            f32::NAN
        }
    }

    fn calibrated_humidity_rh(&self, i2c: &mut I) -> f32 {
        let t_out = Self::read_h_out(i2c);

        //let calibration = self.calibration.temperature.as_ref().unwrap();
        //let t_degc = calibration.t0_degc as f32 + (calibration.slope * (t_out - calibration.t0_out) as f32);

        //t_degc
        0.0
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

impl<I: WriteRead + Read + Write + 'static> Bind<Mutex<I>> for Sensor<I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    fn on_bind(&'static mut self, address: Address<Mutex<I>>) {
        self.i2c.replace(address);
    }
}

//pub struct Initialize;

impl<I: WriteRead + Read + Write> NotificationHandler<Lifecycle> for Sensor<I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    fn on_notification(&'static mut self, event: Lifecycle) -> Completion {
        log::info!("[hts221] Lifecycle: {:?}", event);
        match event {
            Lifecycle::Initialize => { self.initialize() }
            Lifecycle::Start => { self.start() }
            Lifecycle::Stop => { Completion::immediate() }
            Lifecycle::Sleep => { Completion::immediate() }
            Lifecycle::Hibernate => { Completion::immediate() }
        }
    }
}

impl<I: WriteRead + Read + Write> NotificationHandler<DataReady> for Sensor<I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    fn on_notification(&'static mut self, message: DataReady) -> Completion {
        Completion::defer(async move {
            if self.i2c.is_some() {
                let mut i2c = self.i2c.as_ref().unwrap().lock().await;
                let temp_degc = self.calibrated_temperature_degc(&mut i2c);
                let humidity_rh = self.calibrated_humidity_rh(&mut i2c);
                log::info!("[hts221] temperature is {} °F", Self::c_to_f(temp_degc));
            }
            /*
            if let Some(ref i2c) = self.i2c {
                let mut i2c = i2c.lock().await;
                let h = Self::read_h_out(&mut i2c);
                let temp_degc = Self::calibrated_temperature_degc(
                    self.calibration.temperature.as_ref().unwrap(),
                    &mut i2c
                );
                log::info!("[hts221] temperature is {} °F", Self::c_to_f(temp_degc));
                Self::modify_ctrl_reg3(&mut i2c, |mut reg| {
                    reg.enable = true;
                    reg
                });
            }
             */
        })
    }
}


impl<I: WriteRead + Read + Write + 'static> Address<Sensor<I>>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    pub fn signal_data_ready(&self) {
        self.notify(DataReady)
    }
}