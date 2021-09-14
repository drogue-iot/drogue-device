use super::{Radio, RadioIrq, RadioPhyEvent};
use crate::traits::lora::LoraError as DriverError;
use embassy::util::Unborrow;
use embassy_hal_common::unborrow;
use embassy_stm32::{
    dma::NoDma,
    gpio::{AnyPin, Output},
    interrupt::SUBGHZ_RADIO,
    subghz::{
        self, BitSync, CalibrateImage, CfgIrq, CodingRate, HeaderType, Irq, LoRaBandwidth,
        LoRaModParams, LoRaPacketParams, LoRaSyncWord, Ocp, PaConfig, PaSel, PacketType, RampTime,
        RegMode, RfFreq, SpreadingFactor as SF, StandbyClk, SubGhz, TcxoMode, TcxoTrim, Timeout,
        TxParams,
    },
};
use embedded_hal::digital::v2::OutputPin;
use lorawan_device::{
    radio::{
        Bandwidth, Error as LoraError, Event as LoraEvent, PhyRxTx, Response as LoraResponse,
        RxQuality, SpreadingFactor,
    },
    Timings,
};

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum State {
    Idle,
    Txing,
    Rxing,
}

#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct RadioError;

pub struct SubGhzRadio<'a> {
    radio: SubGhz<'a, NoDma, NoDma>,
    switch: RadioSwitch<'a>,
    rx_buffer: &'a mut [u8],
    rx_buffer_written: usize,
    radio_state: State,
}

fn convert_spreading_factor(sf: SpreadingFactor) -> SF {
    match sf {
        SpreadingFactor::_7 => SF::Sf7,
        SpreadingFactor::_8 => SF::Sf8,
        SpreadingFactor::_9 => SF::Sf9,
        SpreadingFactor::_10 => SF::Sf10,
        SpreadingFactor::_11 => SF::Sf11,
        SpreadingFactor::_12 => SF::Sf12,
    }
}

fn convert_bandwidth(bw: Bandwidth) -> LoRaBandwidth {
    match bw {
        Bandwidth::_125KHz => LoRaBandwidth::Bw125,
        Bandwidth::_250KHz => LoRaBandwidth::Bw250,
        Bandwidth::_500KHz => LoRaBandwidth::Bw500,
    }
}

pub struct RadioSwitch<'a> {
    ctrl1: Output<'a, AnyPin>,
    ctrl2: Output<'a, AnyPin>,
    ctrl3: Output<'a, AnyPin>,
}

impl<'a> RadioSwitch<'a> {
    pub fn new(
        ctrl1: Output<'a, AnyPin>,
        ctrl2: Output<'a, AnyPin>,
        ctrl3: Output<'a, AnyPin>,
    ) -> Self {
        Self {
            ctrl1,
            ctrl2,
            ctrl3,
        }
    }

    pub fn set_rx(&mut self) {
        self.ctrl1.set_high();
        self.ctrl2.set_low();
        self.ctrl3.set_high();
    }

    fn set_tx_lp(&mut self) {
        self.ctrl1.set_high();
        self.ctrl2.set_high();
        self.ctrl3.set_high();
    }

    fn set_tx_hp(&mut self) {
        self.ctrl2.set_high();
        self.ctrl1.set_low();
        self.ctrl3.set_high();
    }
}

impl<'a> SubGhzRadio<'a> {
    pub fn new(
        radio: SubGhz<'a, NoDma, NoDma>,
        switch: RadioSwitch<'a>,
        rx_buffer: &'a mut [u8],
    ) -> Self {
        Self {
            radio,
            switch,
            rx_buffer,
            rx_buffer_written: 0,
            radio_state: State::Idle,
        }
    }

    pub fn check_status(&mut self) -> Result<(), RadioError> {
        //let status = self.radio.status()?;
        //trace!("CMD: {:?}, MODE: {:?}", status.cmd(), status.mode());
        Ok(())
    }

    pub fn configure(&mut self) -> Result<(), RadioError> {
        info!("Initializing STM32WL SUBGHZ radio");
        self.radio.set_standby(StandbyClk::Rc)?;
        self.check_status()?;
        let tcxo_mode = TcxoMode::new()
            .set_txco_trim(TcxoTrim::Volts1pt7)
            .set_timeout(Timeout::from_duration_sat(
                core::time::Duration::from_millis(100),
            ));

        self.radio.set_tcxo_mode(&tcxo_mode)?;
        self.radio.set_regulator_mode(RegMode::Smps)?;
        self.radio.calibrate(0x7F)?;

        while self.radio.rfbusys() {}

        self.radio.calibrate_image(CalibrateImage::ISM_863_870)?;

        self.radio.set_buffer_base_address(0, 0)?;

        let pa_config = PaConfig::new()
            .set_pa(PaSel::Lp)
            .set_hp_max(0x0)
            .set_pa_duty_cycle(0x1);

        self.radio.set_pa_config(&pa_config)?;

        self.radio.set_pa_ocp(Ocp::Max140m)?;

        let tx_params = TxParams::new()
            .set_ramp_time(RampTime::Micros40)
            .set_power(0x0D);
        self.radio.set_tx_params(&tx_params)?;

        self.radio.set_packet_type(PacketType::LoRa)?;
        self.check_status()?;
        self.radio.set_lora_sync_word(LoRaSyncWord::Public)?;
        self.check_status()?;
        info!("Done initializing STM32WL SUBGHZ radio");
        Ok(())
    }

    fn handle_event(&mut self, event: LoraEvent<Self>) -> Result<LoraResponse<Self>, Error<'a>>
    where
        Self: core::marker::Sized,
    {
        let (new_state, response) = match &self.radio_state {
            State::Idle => match event {
                LoraEvent::TxRequest(config, buf) => {
                    info!("TX with payload len {}", buf.len());
                    self.configure()?;

                    self.radio
                        .set_rf_frequency(&RfFreq::from_frequency(config.rf.frequency))?;

                    let packet_params = LoRaPacketParams::new()
                        .set_preamble_len(8)
                        .set_header_type(HeaderType::Variable)
                        .set_payload_len(buf.len() as u8)
                        .set_crc_en(true)
                        .set_invert_iq(false);

                    self.radio.set_lora_packet_params(&packet_params);

                    let mod_params = LoRaModParams::new()
                        .set_sf(convert_spreading_factor(config.rf.spreading_factor))
                        .set_bw(convert_bandwidth(config.rf.bandwidth))
                        .set_cr(CodingRate::Cr45)
                        .set_ldro_en(false);
                    self.radio.set_lora_mod_params(&mod_params)?;

                    self.switch.set_tx_lp();

                    let irq_cfg = CfgIrq::new()
                        .irq_enable_all(Irq::TxDone)
                        .irq_enable_all(Irq::Timeout);
                    self.radio.set_irq_cfg(&irq_cfg)?;

                    self.radio.set_buffer_base_address(0, 0)?;
                    self.radio.write_buffer(0, buf)?;

                    self.radio.set_tx(Timeout::DISABLED)?;
                    self.check_status()?;

                    trace!("TX STARTED");

                    (State::Txing, Ok(LoraResponse::Txing))
                }
                LoraEvent::RxRequest(config) => {
                    self.configure()?;
                    let packet_params = LoRaPacketParams::new()
                        .set_preamble_len(8)
                        .set_header_type(HeaderType::Variable)
                        .set_payload_len(0xFF)
                        .set_crc_en(true)
                        .set_invert_iq(true);
                    self.radio.set_lora_packet_params(&packet_params)?;
                    self.radio
                        .set_rf_frequency(&RfFreq::from_frequency(config.frequency))?;

                    let mod_params = LoRaModParams::new()
                        .set_sf(convert_spreading_factor(config.spreading_factor))
                        .set_bw(convert_bandwidth(config.bandwidth))
                        .set_cr(CodingRate::Cr45)
                        .set_ldro_en(true);
                    self.radio.set_lora_mod_params(&mod_params)?;

                    self.switch.set_rx();

                    let irq_cfg = CfgIrq::new()
                        .irq_enable_all(Irq::RxDone)
                        .irq_enable_all(Irq::Timeout);
                    self.radio.set_irq_cfg(&irq_cfg)?;

                    self.radio.set_rx(Timeout::DISABLED)?;

                    (State::Rxing, Ok(LoraResponse::Rxing))
                }
                LoraEvent::PhyEvent(_) => {
                    (State::Idle, Err(Error(LoraError::PhyError(RadioError))))
                }
                LoraEvent::CancelRx => (State::Idle, Err(Error(LoraError::CancelRxWhileIdle))),
            },
            State::Txing => match event {
                LoraEvent::PhyEvent(phyevent) => match phyevent {
                    RadioPhyEvent::Irq => {
                        trace!("IRQ EVENT");
                        //self.radio.set_mode(RadioMode::Stdby).ok().unwrap();
                        let (_, irq_status) = self.radio.irq_status()?;
                        self.radio.clear_irq_status(irq_status)?;
                        if irq_status & Irq::TxDone.mask() != 0 {
                            let stats = self.radio.lora_stats()?;
                            let (status, error_mask) = self.radio.op_error()?;
                            trace!(
                                "TX done. Stats: {:?}. OP error: {:?}, mask {:?}",
                                stats,
                                status,
                                error_mask
                            );
                            (State::Idle, Ok(LoraResponse::TxDone(0)))
                        } else if irq_status & Irq::Timeout.mask() != 0 {
                            trace!("TX timeout");
                            (State::Idle, Err(Error(LoraError::PhyError(RadioError))))
                        } else {
                            (State::Txing, Ok(LoraResponse::Txing))
                        }
                    }
                },
                LoraEvent::TxRequest(_, _) => {
                    (State::Txing, Err(Error(LoraError::TxRequestDuringTx)))
                }
                LoraEvent::RxRequest(_) => (State::Txing, Err(Error(LoraError::RxRequestDuringTx))),
                LoraEvent::CancelRx => (State::Txing, Err(Error(LoraError::CancelRxDuringTx))),
            },
            State::Rxing => match event {
                LoraEvent::PhyEvent(phyevent) => match phyevent {
                    RadioPhyEvent::Irq => {
                        trace!("RX IRQ EVENT");
                        //self.radio.set_mode(RadioMode::Stdby).ok().unwrap();
                        let (_, irq_status) = self.radio.irq_status()?;
                        self.radio.clear_irq_status(irq_status)?;
                        if irq_status & Irq::RxDone.mask() != 0 {
                            let (status, len, ptr) = self.radio.rx_buffer_status()?;
                            trace!(
                                "RX done. Received {} bytes. Status: {:?}",
                                len,
                                status.cmd()
                            );
                            let packet_status = self.radio.lora_packet_status()?;
                            let rssi = packet_status.rssi_pkt().to_integer();
                            let snr = packet_status.snr_pkt().to_integer();
                            self.radio
                                .read_buffer(ptr, &mut self.rx_buffer[..len as usize])?;
                            self.rx_buffer_written = len as usize;
                            (
                                State::Idle,
                                Ok(LoraResponse::RxDone(RxQuality::new(rssi, snr as i8))),
                            )
                        } else if irq_status & Irq::Timeout.mask() != 0 {
                            trace!("RX timeout");
                            (State::Idle, Err(Error(LoraError::PhyError(RadioError))))
                        } else {
                            (State::Rxing, Ok(LoraResponse::Rxing))
                        }
                    }
                },
                LoraEvent::TxRequest(_, _) => {
                    (State::Rxing, Err(Error(LoraError::TxRequestDuringTx)))
                }
                LoraEvent::RxRequest(_) => (State::Rxing, Err(Error(LoraError::RxRequestDuringRx))),
                LoraEvent::CancelRx => {
                    self.radio.set_standby(StandbyClk::Rc)?;
                    (State::Idle, Ok(LoraResponse::Idle))
                }
            },
        };
        self.radio_state = new_state;
        response
    }
}

impl<'a> PhyRxTx for SubGhzRadio<'a> {
    type PhyEvent = RadioPhyEvent;
    type PhyError = RadioError;
    type PhyResponse = ();

    fn get_mut_radio(&mut self) -> &mut Self {
        self
    }

    fn get_received_packet(&mut self) -> &mut [u8] {
        &mut self.rx_buffer[..self.rx_buffer_written]
    }

    fn handle_event(
        &mut self,
        event: LoraEvent<Self>,
    ) -> Result<LoraResponse<Self>, LoraError<Self>>
    where
        Self: core::marker::Sized,
    {
        Ok(SubGhzRadio::handle_event(self, event)?)
    }
}

struct Error<'a>(LoraError<SubGhzRadio<'a>>);

impl<'a> From<RadioError> for Error<'a> {
    fn from(f: RadioError) -> Self {
        Error(LoraError::PhyError(RadioError))
    }
}

impl<'a> From<embassy_stm32::spi::Error> for Error<'a> {
    fn from(f: embassy_stm32::spi::Error) -> Self {
        Error(LoraError::PhyError(RadioError))
    }
}

impl<'a> From<embassy_stm32::spi::Error> for RadioError {
    fn from(f: embassy_stm32::spi::Error) -> Self {
        RadioError
    }
}

impl<'a> From<Error<'a>> for LoraError<SubGhzRadio<'a>> {
    fn from(f: Error<'a>) -> Self {
        f.0
    }
}

impl<'a> From<RadioError> for LoraError<SubGhzRadio<'a>> {
    fn from(f: RadioError) -> Self {
        LoraError::PhyError(f)
    }
}

impl<'a> From<RadioError> for crate::traits::lora::LoraError {
    fn from(f: RadioError) -> Self {
        crate::traits::lora::LoraError::OtherError
    }
}

impl<'a> Timings for SubGhzRadio<'a> {
    fn get_rx_window_offset_ms(&self) -> i32 {
        -500
    }
    fn get_rx_window_duration_ms(&self) -> u32 {
        800
    }
}

impl<'a> Radio for SubGhzRadio<'a> {
    fn reset(&mut self) -> Result<(), DriverError> {
        self.radio.reset();
        self.configure()?;
        Ok(())
    }
}

use embassy::util::InterruptFuture;
impl RadioIrq for SUBGHZ_RADIO {
    #[rustfmt::skip]
    type Future<'m> = InterruptFuture<'m, SUBGHZ_RADIO>;
    fn wait<'m>(&'m mut self) -> Self::Future<'m> {
        InterruptFuture::new(self)
    }
}

/*
use core::future::Future;
use embassy::interrupt::InterruptExt;
use embassy::util::InterruptFuture;
use embassy::util::Signal;

pub struct SubGhzRadioIrq {
    signal: Signal<()>,
    //  irq: SUBGHZ_RADIO,
}

use core::ptr;
impl SubGhzRadioIrq {
    pub fn new() -> Self {
        //irq: SUBGHZ_RADIO) -> Self {
        /*
        irq}.disable();
        irq.set_handler(|_| {
            trace!("RADIO IRQ");
        });
        irq.set_handler_context(ptr::null_mut());
        irq.unpend();
        irq.enable();
        */
        Self {
            //   irq,
            signal: Signal::new(),
        }
    }
}

impl RadioIrq for SubGhzRadioIrq {
    #[rustfmt::skip]
    type Future<'m> = impl Future<Output = ()> + 'm;
    fn wait<'m>(&'m mut self) -> Self::Future<'m> {
        async move { self.signal.wait().await }
    }
}
*/
