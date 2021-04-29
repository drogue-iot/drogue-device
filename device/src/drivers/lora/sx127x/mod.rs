use crate::fmt::*;
use crate::time::*;
use crate::traits::gpio::WaitForRisingEdge;
use crate::traits::lora::*;
use core::future::Future;
use embedded_hal::{
    blocking::{
        delay::DelayMs,
        spi::{Transfer, Write},
    },
    digital::v2::OutputPin,
};

use lorawan_device::{
    radio, region, Device as LorawanDevice, Error as LorawanError, Event as LorawanEvent,
    Response as LorawanResponse,
};
use lorawan_encoding::default_crypto::DefaultFactory as Crypto;

mod sx127x_lora;
mod sx127x_radio;

use sx127x_radio::{RadioPhyEvent, Sx127xRadio as Radio};

enum DriverState<SPI, CS, RESET, E>
where
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    E: 'static,
{
    Initialized(Radio<SPI, CS, RESET, E>),
    Configured(LorawanDevice<Radio<SPI, CS, RESET, E>, Crypto>),
}

pub struct Sx127xDriver<'a, P, SPI, CS, RESET, E>
where
    P: WaitForRisingEdge,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'static,
    CS: OutputPin + 'static,
    RESET: OutputPin + 'static,
    E: 'static,
{
    irq: P,
    state: Option<DriverState<SPI, CS, RESET, E>>,
    get_random: fn() -> u32,
    _phantom: core::marker::PhantomData<&'a SPI>,
}

pub enum DriverEvent {
    ProcessAfter(u32),
    JoinSuccess,
    JoinFailed,
    SessionExpired,
    Ack,
    AckWithData(usize, [u8; 255]),
    AckTimeout,
    None,
}

impl<'a, P, SPI, CS, RESET, E> Sx127xDriver<'a, P, SPI, CS, RESET, E>
where
    P: WaitForRisingEdge,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'a,
    CS: OutputPin + 'a,
    RESET: OutputPin + 'a,
{
    pub fn new(
        irq: P,
        spi: SPI,
        cs: CS,
        reset: RESET,
        delay: &mut dyn DelayMs<u8>,
        get_random: fn() -> u32,
    ) -> Result<Self, LoraError> {
        crate::log_stack!();
        let radio = Radio::new(spi, cs, reset, delay)?;
        Ok(Self {
            irq,
            state: Some(DriverState::Initialized(radio)),
            _phantom: core::marker::PhantomData,
            get_random,
        })
    }

    fn process_event(&mut self, event: LorawanEvent<'a, Radio<SPI, CS, RESET, E>>) -> DriverEvent {
        crate::log_stack!();
        match self.state.take().unwrap() {
            DriverState::Configured(lorawan) => {
                match &event {
                    LorawanEvent::NewSessionRequest => {
                        trace!("New Session Request");
                    }
                    LorawanEvent::RadioEvent(e) => match e {
                        radio::Event::TxRequest(_, _) => (),
                        radio::Event::RxRequest(_) => (),
                        radio::Event::CancelRx => (),
                        radio::Event::PhyEvent(_) => {
                            trace!("Phy event");
                        }
                    },
                    LorawanEvent::TimeoutFired => (),
                    LorawanEvent::SendDataRequest(_e) => {
                        trace!("SendData");
                    }
                }
                crate::log_stack!();
                let (mut new_state, response) = lorawan.handle_event(event);
                trace!("Event handled");
                let event = self.process_response(&mut new_state, response);
                self.state.replace(DriverState::Configured(new_state));
                event
            }
            s => {
                trace!("Not yet configured, event processing skipped");
                self.state.replace(s);
                DriverEvent::None
            }
        }
    }

    fn process_response(
        &self,
        lorawan: &mut LorawanDevice<Radio<SPI, CS, RESET, E>, Crypto>,
        response: Result<LorawanResponse, LorawanError<Radio<SPI, CS, RESET, E>>>,
    ) -> DriverEvent {
        crate::log_stack!();
        match response {
            Ok(response) => match response {
                LorawanResponse::TimeoutRequest(ms) => {
                    trace!("TimeoutRequest: {:?}", ms);
                    return DriverEvent::ProcessAfter(ms);
                }
                LorawanResponse::JoinSuccess => {
                    return DriverEvent::JoinSuccess;
                }
                LorawanResponse::ReadyToSend => {
                    trace!("RxWindow expired but no ACK expected. Ready to Send");
                }
                LorawanResponse::DownlinkReceived(fcnt_down) => {
                    if let Some(downlink) = lorawan.take_data_downlink() {
                        use lorawan_encoding::parser::FRMPayload;

                        if let Ok(FRMPayload::Data(data)) = downlink.frm_payload() {
                            trace!(
                                "Downlink received \t\t(FCntDown={}\tFRM: {:?})",
                                fcnt_down,
                                data,
                            );
                            let mut buf = [0; 255];
                            buf[0..data.len()].copy_from_slice(&data[0..data.len()]);
                            return DriverEvent::AckWithData(data.len(), buf);
                        } else {
                            trace!("Downlink received \t\t(FcntDown={})", fcnt_down);
                            return DriverEvent::Ack;
                        }

                        /*
                        let fhdr = downlink.fhdr();
                        let fopts = fhdr.fopts();
                        let mut mac_commands_len = 0;
                        for mac_command in fopts {
                            if mac_commands_len == 0 {
                                trace!("\tFOpts: ");
                            }
                            trace!("{:?},", mac_command);
                            mac_commands_len += 1;
                        }
                        */
                    }
                }
                LorawanResponse::NoAck => {
                    trace!("RxWindow expired, expected ACK to confirmed uplink not received");
                    return DriverEvent::AckTimeout;
                }
                LorawanResponse::NoJoinAccept => {
                    trace!("No Join Accept Received. Retrying.");
                    return DriverEvent::JoinFailed;
                }
                LorawanResponse::SessionExpired => {
                    trace!("SessionExpired. Created new Session");
                    return DriverEvent::SessionExpired;
                }
                LorawanResponse::NoUpdate => {
                    // info!("No update");
                }
                LorawanResponse::UplinkSending(fcnt_up) => {
                    trace!("Uplink with FCnt {}", fcnt_up);
                }
                LorawanResponse::JoinRequestSending => {
                    trace!("Join Request Sending");
                }
            },
            Err(err) => match err {
                LorawanError::Radio(_) => error!("Radio error"),
                LorawanError::Session(_) => error!("Session error"), //{:?}", e),
                LorawanError::NoSession(_) => error!("NoSession error"),
            },
        }
        DriverEvent::None
    }

    async fn join(&mut self) -> Result<(), LoraError> {
        let mut event: DriverEvent = self.process_event(LorawanEvent::NewSessionRequest);
        loop {
            match event {
                DriverEvent::ProcessAfter(ms) => {
                    let interrupt = self.irq.wait_for_rising_edge();
                    match with_timeout(Duration::from_millis(ms.into()), interrupt).await {
                        Ok(_) => {
                            event = self.process_event(LorawanEvent::RadioEvent(
                                radio::Event::PhyEvent(RadioPhyEvent::Irq),
                            ));
                        }
                        Err(TimeoutError) => {
                            event = self.process_event(LorawanEvent::TimeoutFired);
                        }
                    }
                }
                DriverEvent::JoinSuccess => {
                    return Ok(());
                }
                DriverEvent::JoinFailed => {
                    return Err(LoraError::JoinError);
                }
                _ => {
                    // Wait for interrupt
                    self.irq.wait_for_rising_edge().await;
                    event = self.process_event(LorawanEvent::RadioEvent(radio::Event::PhyEvent(
                        RadioPhyEvent::Irq,
                    )));
                }
            }
        }
    }

    async fn send_data(
        &mut self,
        qos: QoS,
        port: Port,
        data: &[u8],
    ) -> Result<DriverEvent, LoraError> {
        match self.state.take().unwrap() {
            DriverState::Configured(lorawan) => {
                let ready_to_send = lorawan.ready_to_send_data();
                if ready_to_send {
                    let (mut new_state, response) = lorawan.send(
                        data,
                        port,
                        match qos {
                            QoS::Confirmed => true,
                            QoS::Unconfirmed => false,
                        },
                    );
                    let event = self.process_response(&mut new_state, response);
                    self.state.replace(DriverState::Configured(new_state));
                    Ok(event)
                } else {
                    self.state.replace(DriverState::Configured(lorawan));
                    Err(LoraError::NotReady)
                }
            }
            other => {
                //info!("Driver not yet initialized, ignoring configuration");
                self.state.replace(other);
                Err(LoraError::OtherError)
            }
        }
    }

    async fn send_recv(
        &mut self,
        qos: QoS,
        port: Port,
        data: &[u8],
        rx: Option<&mut [u8]>,
    ) -> Result<usize, LoraError> {
        // Await response
        let mut event = self.send_data(qos, port, data).await?;
        loop {
            match event {
                DriverEvent::ProcessAfter(ms) => {
                    let interrupt = self.irq.wait_for_rising_edge();
                    match with_timeout(Duration::from_millis(ms.into()), interrupt).await {
                        Ok(_) => {
                            event = self.process_event(LorawanEvent::RadioEvent(
                                radio::Event::PhyEvent(RadioPhyEvent::Irq),
                            ));
                        }
                        Err(TimeoutError) => {
                            event = self.process_event(LorawanEvent::TimeoutFired);
                        }
                    }
                }
                DriverEvent::AckWithData(len, buf) => {
                    trace!("Received {} bytes of data", len);
                    if let Some(rx) = rx {
                        rx[0..len].copy_from_slice(&buf[0..len]);
                    }
                    return Ok(len);
                }
                DriverEvent::AckTimeout => {
                    trace!("Ack timed out!");
                    return Err(LoraError::AckTimeout);
                }
                DriverEvent::Ack => {
                    trace!("Ack received!");
                    return Ok(0);
                }
                _ => {
                    // Wait for interrupt
                    self.irq.wait_for_rising_edge().await;
                    event = self.process_event(LorawanEvent::RadioEvent(radio::Event::PhyEvent(
                        RadioPhyEvent::Irq,
                    )));
                }
            }
        }
    }
}

impl<'a, P, SPI, CS, RESET, E> LoraDriver for Sx127xDriver<'a, P, SPI, CS, RESET, E>
where
    P: WaitForRisingEdge + 'a,
    SPI: Transfer<u8, Error = E> + Write<u8, Error = E> + 'a,
    E: 'a,
    CS: OutputPin + 'a,
    RESET: OutputPin + 'a,
{
    #[rustfmt::skip]
    type ConfigureFuture<'m> where 'a: 'm = impl Future<Output = Result<(), LoraError>> + 'm;
    fn configure<'m>(&'m mut self, config: &'m LoraConfig) -> Self::ConfigureFuture<'m> {
        async move {
            match self.state.take().unwrap() {
                DriverState::Initialized(radio) => {
                    //info!("Configuring radio");
                    let dev_eui = config.device_eui.as_ref().expect("device EUI must be set");
                    let app_eui = config.app_eui.as_ref().expect("app EUI must be set");
                    let app_key = config.app_key.as_ref().expect("app KEY must be set");
                    //info!("Creating device");
                    let region = to_region(config.region.unwrap_or(LoraRegion::EU868));
                    if let Err(e) = region {
                        return Err(e);
                    }
                    let mut region = region.unwrap();
                    region.set_receive_delay1(5000);
                    let mut lorawan: LorawanDevice<Radio<SPI, CS, RESET, E>, Crypto> =
                        LorawanDevice::new(
                            region,
                            radio,
                            dev_eui.reverse().into(),
                            app_eui.reverse().into(),
                            app_key.clone().into(),
                            self.get_random,
                        );
                    lorawan.set_datarate(region::DR::_3); // Use lower datarate that seems more stable
                    self.state.replace(DriverState::Configured(lorawan));
                    Ok(())
                }
                other => {
                    //info!("Driver not yet initialized, ignoring configuration");
                    self.state.replace(other);
                    Err(LoraError::OtherError)
                }
            }
        }
    }

    #[rustfmt::skip]
    type JoinFuture<'m> where 'a: 'm = impl Future<Output = Result<(), LoraError>> + 'm;
    fn join<'m>(&'m mut self, _: ConnectMode) -> Self::JoinFuture<'m> {
        async move { self.join().await }
    }
    /*
    fn reset(self, message: Reset) -> Response<Self, Result<(), LoraError>> {
        Response::immediate(self, Err(LoraError::OtherError))
    }
    */
    #[rustfmt::skip]
    type SendFuture<'m> where 'a: 'm = impl Future<Output = Result<(), LoraError>> + 'm;
    fn send<'m>(&'m mut self, qos: QoS, port: Port, data: &'m [u8]) -> Self::SendFuture<'m> {
        async move { self.send_recv(qos, port, data, None).await.map(|_| ()) }
    }

    #[rustfmt::skip]
    type SendRecvFuture<'m> where 'a: 'm = impl Future<Output = Result<usize, LoraError>> + 'm;
    fn send_recv<'m>(
        &'m mut self,
        qos: QoS,
        port: Port,
        data: &'m [u8],
        rx: &'m mut [u8],
    ) -> Self::SendRecvFuture<'m> {
        async move { self.send_recv(qos, port, data, Some(rx)).await }
    }
}

fn to_region(region: LoraRegion) -> Result<region::Configuration, LoraError> {
    match region {
        LoraRegion::EU868 => Ok(region::EU868::default().into()),
        LoraRegion::US915 => Ok(region::US915::default().into()),
        LoraRegion::CN470 => Ok(region::CN470::default().into()),
        _ => Err(LoraError::UnsupportedRegion),
    }
}
