use core::future::Future;
use core::mem::MaybeUninit;
use embassy::channel::signal::Signal;
use embassy::interrupt::InterruptExt;
use embassy::util::Unborrow;
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
use lorawan_device::async_device::{
    radio::{Bandwidth, PhyRxTx, RfConfig, RxQuality, SpreadingFactor, TxConfig},
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

pub struct SubGhzState<'a>(MaybeUninit<StateInner<'a>>);
impl<'a> SubGhzState<'a> {
    pub const fn new() -> Self {
        Self(MaybeUninit::uninit())
    }
}

pub struct SubGhzRadio<'a> {
    state: *mut StateInner<'a>,
    irq: SUBGHZ_RADIO,
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
        irq: impl Unborrow<Target = SUBGHZ_RADIO>,
    ) -> Self {
        unborrow!(irq);

        let mut inner = StateInner {
            radio,
            switch,
            radio_state: State::Idle,
        };
        inner.radio.reset();

        let state_ptr = state.0.as_mut_ptr();
        state_ptr.write(inner);

        irq.disable();
        irq.set_handler(|p| {
            // This is safe because we only get interrupts when configured for, so
            // the radio will be awaiting on the signal at this point. If not, the ISR will
            // anyway only adjust the state in the IRQ signal state.
            let state = unsafe { &mut *(p as *mut StateInner<'a>) };
            state.on_interrupt();
        });
        irq.set_handler_context(state_ptr as *mut ());
        irq.enable();

        Self {
            state: state_ptr,
            irq,
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

    async fn do_tx(&mut self, config: TxConfig, buf: &[u8]) -> Result<u32, RadioError> {
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

        loop {
            let RadioPhyEvent::Irq(status, irq_status) = IRQ.wait().await;
            IRQ.reset();

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

                return Ok(0);
            } else if irq_status & Irq::Timeout.mask() != 0 {
                trace!("TX timeout");
                return Err(RadioError);
            }
        }
    }

    async fn do_rx(
        &mut self,
        config: RfConfig,
        buf: &mut [u8],
    ) -> Result<(usize, RxQuality), RadioError> {
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

        loop {
            let RadioPhyEvent::Irq(status, irq_status) = IRQ.wait().await;
            IRQ.reset();
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
                self.radio.read_buffer(ptr, &mut buf[..len as usize])?;
                self.radio.set_standby(StandbyClk::Rc)?;
                return Ok((len as usize, RxQuality::new(rssi, snr as i8)));
            } else if irq_status & Irq::Timeout.mask() != 0 {
                return Err(RadioError);
            } else if irq_status & Irq::TxDone.mask() != 0 {
                return Err(RadioError);
            }
        }
    }

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

impl PhyRxTx for SubGhzRadio<'static> {
    type PhyError = RadioError;

    //    type TxFuture<'m> where 'a: 'm = impl Future<Output = Result<u32, LoraError<SubGhzRadio<'a>>>> + 'm;
    type TxFuture<'m> = impl Future<Output = Result<u32, Self::PhyError>> + 'm;
    fn tx<'m>(&'m mut self, config: TxConfig, buf: &'m [u8]) -> Self::TxFuture<'m> {
        trace!("TX START");
        async move {
            let inner = unsafe { &mut *self.state };
            inner.do_tx(config, buf).await
        }
    }

    type RxFuture<'m> = impl Future<Output = Result<(usize, RxQuality), Self::PhyError>> + 'm;
    fn rx<'m>(&'m mut self, config: RfConfig, buf: &'m mut [u8]) -> Self::RxFuture<'m> {
        trace!("RX START");
        async move {
            let inner = unsafe { &mut *self.state };
            inner.do_rx(config, buf).await
        }
    }
}

impl<'a> From<embassy_stm32::spi::Error> for RadioError {
    fn from(_: embassy_stm32::spi::Error) -> Self {
        RadioError
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
