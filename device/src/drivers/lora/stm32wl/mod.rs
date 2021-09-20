use super::{Radio, RadioIrq};
use crate::traits::lora::LoraError as DriverError;
use embassy::util::Unborrow;
use embassy_hal_common::peripheral::{PeripheralMutex, PeripheralState, StateStorage};
use embassy_hal_common::unborrow;
use embassy_stm32::{
    dma::NoDma,
    gpio::{AnyPin, Output},
    interrupt::SUBGHZ_RADIO,
    subghz::{
        CalibrateImage, CfgIrq, CodingRate, HeaderType, Irq, LoRaBandwidth, LoRaModParams,
        LoRaPacketParams, LoRaSyncWord, Ocp, PaConfig, PaSel, PacketType, RampTime, RegMode,
        RfFreq, SpreadingFactor as SF, StandbyClk, Status, SubGhz, TcxoMode, TcxoTrim, Timeout,
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
pub enum RadioPhyEvent {
    Irq(Status, u16),
}

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

static IRQ: Signal<RadioPhyEvent> = Signal::new();

struct StateInner<'a> {
    radio: SubGhz<'a, NoDma, NoDma>,
    switch: RadioSwitch<'a>,
    radio_state: State,
}

pub struct SubGhzState<'a>(StateStorage<StateInner<'a>>);
impl<'a> SubGhzState<'a> {
    pub const fn new() -> Self {
        Self(StateStorage::new())
    }
}

pub struct SubGhzRadio<'a> {
    rx_buffer: &'a mut [u8],
    rx_buffer_written: usize,
    inner: PeripheralMutex<'a, StateInner<'a>>,
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
        self.ctrl1.set_high().unwrap();
        self.ctrl2.set_low().unwrap();
        self.ctrl3.set_high().unwrap();
    }

    fn set_tx_lp(&mut self) {
        self.ctrl1.set_high().unwrap();
        self.ctrl2.set_high().unwrap();
        self.ctrl3.set_high().unwrap();
    }

    fn set_tx_hp(&mut self) {
        self.ctrl2.set_high().unwrap();
        self.ctrl1.set_low().unwrap();
        self.ctrl3.set_high().unwrap();
    }
}

impl<'a> SubGhzRadio<'a> {
    pub unsafe fn new(
        state: &'a mut SubGhzState<'a>,
        radio: SubGhz<'a, NoDma, NoDma>,
        switch: RadioSwitch<'a>,
        rx_buffer: &'a mut [u8],
        irq: impl Unborrow<Target = SUBGHZ_RADIO>,
    ) -> Self {
        unborrow!(irq);

        Self {
            rx_buffer,
            rx_buffer_written: 0,
            inner: PeripheralMutex::new_unchecked(irq, &mut state.0, move || StateInner {
                radio,
                switch,
                radio_state: State::Idle,
            }),
        }
    }
}

impl<'a> StateInner<'a> {
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
                core::time::Duration::from_millis(40),
            ));

        self.radio.set_tcxo_mode(&tcxo_mode)?;
        self.radio.set_regulator_mode(RegMode::Ldo)?;

        self.radio.calibrate_image(CalibrateImage::ISM_863_870)?;

        self.radio.set_buffer_base_address(0, 0)?;

        self.radio.set_pa_config(
            &PaConfig::new()
                .set_pa_duty_cycle(0x1)
                .set_hp_max(0x0)
                .set_pa(PaSel::Lp),
        )?;

        self.radio.set_pa_ocp(Ocp::Max140m)?;

        //        let tx_params = TxParams::LP_14.set_ramp_time(RampTime::Micros40);
        self.radio.set_tx_params(
            &TxParams::new()
                .set_ramp_time(RampTime::Micros40)
                .set_power(0x0A),
        )?;

        self.radio.set_packet_type(PacketType::LoRa)?;
        self.radio.set_lora_sync_word(LoRaSyncWord::Public)?;
        info!("Done initializing STM32WL SUBGHZ radio");
        Ok(())
    }

    fn handle_event(
        &mut self,
        event: LoraEvent<SubGhzRadio<'a>>,
        rx_buffer: &mut [u8],
        rx_buffer_written: &mut usize,
    ) -> Result<LoraResponse<SubGhzRadio<'a>>, Error<'a>>
    where
        Self: core::marker::Sized,
    {
        let (new_state, response) = match &self.radio_state {
            State::Idle => match event {
                LoraEvent::TxRequest(config, buf) => {
                    //trace!("TX Request: {}", config);
                    self.switch.set_tx_lp();
                    self.configure()?;

                    self.radio
                        .set_rf_frequency(&RfFreq::from_frequency(config.rf.frequency))?;

                    let mod_params = LoRaModParams::new()
                        .set_sf(convert_spreading_factor(config.rf.spreading_factor))
                        .set_bw(convert_bandwidth(config.rf.bandwidth))
                        .set_cr(CodingRate::Cr45)
                        .set_ldro_en(true);
                    self.radio.set_lora_mod_params(&mod_params)?;

                    let packet_params = LoRaPacketParams::new()
                        .set_preamble_len(8)
                        .set_header_type(HeaderType::Variable)
                        .set_payload_len(buf.len() as u8)
                        .set_crc_en(true)
                        .set_invert_iq(false);

                    self.radio.set_lora_packet_params(&packet_params)?;

                    let irq_cfg = CfgIrq::new()
                        .irq_enable_all(Irq::TxDone)
                        .irq_enable_all(Irq::RxDone)
                        .irq_enable_all(Irq::Timeout);
                    self.radio.set_irq_cfg(&irq_cfg)?;

                    self.radio.set_buffer_base_address(0, 0)?;
                    self.radio.write_buffer(0, buf)?;

                    self.radio.set_tx(Timeout::DISABLED)?;
                    self.check_status()?;

                    (State::Txing, Ok(LoraResponse::Txing))
                }
                LoraEvent::RxRequest(config) => {
                    //                   trace!("Starting RX: {}", config);
                    self.switch.set_rx();
                    self.configure()?;

                    self.radio
                        .set_rf_frequency(&RfFreq::from_frequency(config.frequency))?;

                    let mod_params = LoRaModParams::new()
                        .set_sf(convert_spreading_factor(config.spreading_factor))
                        .set_bw(convert_bandwidth(config.bandwidth))
                        .set_cr(CodingRate::Cr45)
                        .set_ldro_en(true);
                    self.radio.set_lora_mod_params(&mod_params)?;

                    let packet_params = LoRaPacketParams::new()
                        .set_preamble_len(8)
                        .set_header_type(HeaderType::Variable)
                        .set_payload_len(0xFF)
                        .set_crc_en(true)
                        .set_invert_iq(true);
                    self.radio.set_lora_packet_params(&packet_params)?;

                    let irq_cfg = CfgIrq::new()
                        .irq_enable_all(Irq::RxDone)
                        .irq_enable_all(Irq::PreambleDetected)
                        .irq_enable_all(Irq::HeaderErr)
                        .irq_enable_all(Irq::Timeout)
                        .irq_enable_all(Irq::Err);
                    self.radio.set_irq_cfg(&irq_cfg)?;

                    self.radio.set_rx(Timeout::DISABLED)?;
                    trace!("RX started");

                    (State::Rxing, Ok(LoraResponse::Rxing))
                }
                LoraEvent::PhyEvent(_) => {
                    (State::Idle, Err(Error(LoraError::PhyError(RadioError))))
                }
                LoraEvent::CancelRx => (State::Idle, Err(Error(LoraError::CancelRxWhileIdle))),
            },
            State::Txing => match event {
                LoraEvent::PhyEvent(phyevent) => match phyevent {
                    RadioPhyEvent::Irq(status, irq_status) => {
                        //self.radio.set_mode(RadioMode::Stdby).ok().unwrap();
                        trace!("TX IRQ {:?}, {:?}", status, irq_status);
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
                    RadioPhyEvent::Irq(status, irq_status) => {
                        //let mut delay = embassy::time::Delay;
                        //use embedded_hal::blocking::delay::DelayMs;
                        //delay.delay_ms(1000 as u32);
                        //self.radio.set_mode(RadioMode::Stdby).ok().unwrap();
                        trace!("RX IRQ {:?}, {:?}", status, irq_status);
                        if irq_status & Irq::RxDone.mask() != 0 {
                            let (status, len, ptr) = self.radio.rx_buffer_status()?;

                            let packet_status = self.radio.lora_packet_status()?;
                            let rssi = packet_status.rssi_pkt().to_integer();
                            let snr = packet_status.snr_pkt().to_integer();
                            trace!(
                                "RX done. Received {} bytes. RX status: {:?}. Pkt status: {:?}",
                                len,
                                status.cmd(),
                                packet_status,
                            );
                            self.radio
                                .read_buffer(ptr, &mut rx_buffer[..len as usize])?;
                            *rx_buffer_written = len as usize;
                            self.radio.set_standby(StandbyClk::Rc)?;
                            (
                                State::Idle,
                                Ok(LoraResponse::RxDone(RxQuality::new(rssi, snr as i8))),
                            )
                        } else if irq_status & Irq::Timeout.mask() != 0 {
                            //  trace!("RX timeout");
                            //   self.radio.set_standby(StandbyClk::Rc)?;
                            //    (State::Idle, Err(Error(LoraError::PhyError(RadioError))))
                            (State::Idle, Err(Error(LoraError::PhyError(RadioError))))
                        } else if irq_status & Irq::TxDone.mask() != 0 {
                            (State::Idle, Err(Error(LoraError::PhyError(RadioError))))
                        } else {
                            trace!("Still RXING");
                            (State::Rxing, Ok(LoraResponse::Rxing))
                        }
                    }
                },
                LoraEvent::TxRequest(_, _) => {
                    (State::Rxing, Err(Error(LoraError::TxRequestDuringTx)))
                }
                LoraEvent::RxRequest(_) => (State::Rxing, Err(Error(LoraError::RxRequestDuringRx))),
                LoraEvent::CancelRx => {
                    trace!("Cancel RX while Rxing");
                    self.radio.set_standby(StandbyClk::Rc)?;
                    (State::Idle, Ok(LoraResponse::Idle))
                }
            },
        };
        self.radio_state = new_state;
        response
    }
}

impl<'a> PeripheralState for StateInner<'a> {
    type Interrupt = SUBGHZ_RADIO;
    fn on_interrupt(&mut self) {
        let (status, irq_status) = self.radio.irq_status().expect("error getting irq status");
        self.radio
            .clear_irq_status(irq_status)
            .expect("error clearing irq status");
        trace!("IRQ {:?}, {:?}", status, irq_status);
        if irq_status & Irq::PreambleDetected.mask() != 0 {
            trace!("Preamble detected, ignoring");
        } else {
            IRQ.signal(RadioPhyEvent::Irq(status, irq_status));
        }
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
        let rx_buffer = &mut self.rx_buffer;
        let rx_buffer_written = &mut self.rx_buffer_written;
        self.inner
            .with(|state| Ok(state.handle_event(event, rx_buffer, rx_buffer_written)?))
    }
}

struct Error<'a>(LoraError<SubGhzRadio<'a>>);

impl<'a> From<RadioError> for Error<'a> {
    fn from(_: RadioError) -> Self {
        Error(LoraError::PhyError(RadioError))
    }
}

impl<'a> From<embassy_stm32::spi::Error> for Error<'a> {
    fn from(_: embassy_stm32::spi::Error) -> Self {
        Error(LoraError::PhyError(RadioError))
    }
}

impl<'a> From<embassy_stm32::spi::Error> for RadioError {
    fn from(_: embassy_stm32::spi::Error) -> Self {
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
    fn from(_: RadioError) -> Self {
        crate::traits::lora::LoraError::OtherError
    }
}

impl<'a> Timings for SubGhzRadio<'a> {
    fn get_rx_window_offset_ms(&self) -> i32 {
        -200
    }
    fn get_rx_window_duration_ms(&self) -> u32 {
        800
    }
}

impl<'a> Radio for SubGhzRadio<'a> {
    type Interrupt = SubGhzRadioIrq<'static>;
    fn reset(&mut self) -> Result<(), DriverError> {
        self.inner.with(|state| {
            state.radio.reset();
            state.configure()?;
            Ok(())
        })
    }
}

/*
impl RadioIrq for SUBGHZ_RADIO {
    #[rustfmt::skip]
    type Future<'m> = InterruptFuture<'m, SUBGHZ_RADIO>;
    fn wait<'m>(&'m mut self) -> Self::Future<'m> {
        InterruptFuture::new(self)
    }
}

*/
use core::future::Future;
use embassy::channel::signal::Signal;

pub struct SubGhzRadioIrq<'a> {
    signal: &'a Signal<RadioPhyEvent>,
}

impl<'a> SubGhzRadioIrq<'a> {
    pub fn new() -> Self {
        Self { signal: &IRQ }
    }
}

impl RadioIrq<RadioPhyEvent> for SubGhzRadioIrq<'static> {
    #[rustfmt::skip]
    type Future<'m> = impl Future<Output = RadioPhyEvent> + 'm;
    fn wait<'m>(&'m mut self) -> Self::Future<'m> {
        trace!("Waiting for IRQ");
        async move {
            let r = self.signal.wait().await;
            self.signal.reset();
            trace!("IRQ raised");
            r
        }
    }
}
