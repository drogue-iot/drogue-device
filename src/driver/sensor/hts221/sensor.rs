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
use crate::driver::sensor::hts221::register::calibration::*;
use core::default::Default;
use crate::driver::sensor::hts221::register::who_am_i::WhoAmI;
use crate::hal::i2c::I2cAddress;
use crate::driver::sensor::hts221::register::status::Status;
use crate::driver::sensor::hts221::register::t_out::Tout;
use crate::driver::sensor::hts221::register::h_out::Hout;
use crate::driver::sensor::hts221::register::ctrl1::{Ctrl1, OutputDataRate, BlockDataUpdate};
use crate::driver::sensor::hts221::register::{CtrlReg2, CTRL_REG2, CtrlReg3, CTRL_REG3};

pub const ADDR: u8 = 0x5F;

pub struct Sensor<I: WriteRead + Read + Write + 'static>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    address: I2cAddress,
    i2c: Option<Address<Mutex<I>>>,
    calibration: Option<Calibration>,
}

impl<I: WriteRead + Read + Write + 'static> Sensor<I>
    where
        <I as WriteRead>::Error: Debug,
        <I as Write>::Error: Debug,
{
    pub fn new() -> Self {
        Self {
            address: I2cAddress::new( ADDR ),
            i2c: None,
            calibration: None,
        }
    }

    // ------------------------------------------------------------------------
    // Lifecycle
    // ------------------------------------------------------------------------

    fn initialize(&'static mut self) -> Completion {
        Completion::defer(async move {
            if let Some(ref i2c) = self.i2c {
                let mut i2c = i2c.lock().await;

                /*
                Self::modify_ctrl_reg2(&mut i2c, |mut reg| {
                    reg.boot = false;
                    reg.one_shot = true;
                    reg
                });
                 */

                Ctrl1::modify( self.address, &mut i2c, |reg| {
                    reg.power_active()
                        .output_data_rate( OutputDataRate::Hz7 )
                        .block_data_update( BlockDataUpdate::MsbLsbReading );
                });

                Self::modify_ctrl_reg3(&mut i2c, |mut reg| {
                    reg.enable = true;
                    //reg.active = ActiveState::High;
                    //reg.mode = ReadyMode::PushPull;
                    reg
                });

                log::info!("[hts221] address=0x{:X}", WhoAmI::read( self.address, &mut i2c) );
                loop {
                    if ! Status::read( self.address, &mut i2c).any_available() {
                        break
                    }
                    Hout::read(self.address, &mut i2c);
                    Tout::read(self.address, &mut i2c);
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
            self.calibration.replace(Calibration::read( self.address, &mut i2c));
        }
    }

    // ------------------------------------------------------------------------
    // CTRL_REG1
    // ------------------------------------------------------------------------

    /*
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
     */

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

                if let Some(ref calibration) = self.calibration {
                    let t_out = Tout::read(self.address, &mut i2c);
                    let t = calibration.calibrated_temperature( t_out );

                    let h_out = Hout::read(self.address, &mut i2c);
                    let h = calibration.calibrated_humidity( h_out );

                    log::info!("[hts221] temperature={:.2}°F humidity={:.2}%rh", t.into_fahrenheit(), h);
                } else {
                    log::info!("[hts221] no calibration data available")
                }



                //let temp_degc = self.calibrated_temperature_degc(&mut i2c);
                //let humidity_rh = self.calibrated_humidity_rh(&mut i2c);
            }
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