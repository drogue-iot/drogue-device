use crate::traits::lora::{LoraError, *};
use core::future::Future;

use lorawan_device::async_device::{
    radio, region, Device as LorawanDevice, JoinMode as LoraJoinMode, Timings,
};
use lorawan_encoding::{default_crypto::DefaultFactory as Crypto, parser::DevAddr as LDevAddr};
use rand_core::RngCore;

pub trait Radio: radio::PhyRxTx + Timings {}

impl<R: radio::PhyRxTx + Timings> Radio for R {}

pub struct DriverTimer;

impl radio::Timer for DriverTimer {
    type DelayFuture<'m> = impl Future<Output = ()> + 'm;
    fn delay_ms<'m>(&'m mut self, millis: u64) -> Self::DelayFuture<'m> {
        embassy_time::Timer::after(embassy_time::Duration::from_millis(millis))
    }
}

pub struct LoraDevice<R, RNG>
where
    R: Radio,
    RNG: RngCore,
{
    device: LorawanDevice<R, Crypto, DriverTimer, RNG>,
}

const RX_DELAY1: u32 = 5000;
impl<R, RNG> LoraDevice<R, RNG>
where
    R: Radio,
    RNG: RngCore,
{
    pub fn new(config: &LoraConfig, radio: R, rng: RNG) -> Result<Self, LoraError> {
        let data_rate = to_datarate(config.spreading_factor.unwrap_or(SpreadingFactor::SF7));
        let region = to_region(config.region.unwrap_or(LoraRegion::EU868));
        if let Err(e) = region {
            return Err(e);
        }
        let mut region = region.unwrap();
        region.set_receive_delay1(RX_DELAY1);
        let mut device = LorawanDevice::new(region, radio, DriverTimer, rng);
        device.set_datarate(data_rate);
        Ok(Self { device })
    }
}

impl<R, RNG> LoraDriver for LoraDevice<R, RNG>
where
    R: Radio,
    RNG: RngCore,
{
    type JoinFuture<'m> = impl Future<Output = Result<(), LoraError>> + 'm
    where
        Self: 'm;
    fn join<'m>(&'m mut self, mode: JoinMode) -> Self::JoinFuture<'m> {
        let join_mode = to_lorajoinmode(mode);
        async move {
            self.device
                .join(&join_mode)
                .await
                .map_err(|_| LoraError::JoinError)?;
            Ok(())
        }
    }

    type SendFuture<'m> = impl Future<Output = Result<(), LoraError>> + 'm
    where
        Self: 'm;
    fn send<'m>(&'m mut self, qos: QoS, port: Port, data: &'m [u8]) -> Self::SendFuture<'m> {
        async move {
            self.device
                .send(
                    data,
                    port,
                    match qos {
                        QoS::Confirmed => true,
                        QoS::Unconfirmed => false,
                    },
                )
                .await
                .map_err(|_| LoraError::SendError)?;
            Ok(())
        }
    }

    type SendRecvFuture<'m> = impl Future<Output = Result<usize, LoraError>> + 'm
    where
        Self: 'm;
    fn send_recv<'m>(
        &'m mut self,
        qos: QoS,
        port: Port,
        data: &'m [u8],
        rx: &'m mut [u8],
    ) -> Self::SendRecvFuture<'m> {
        async move {
            let len = self
                .device
                .send_recv(
                    data,
                    rx,
                    port,
                    match qos {
                        QoS::Confirmed => true,
                        QoS::Unconfirmed => false,
                    },
                )
                .await
                .map_err(|_| LoraError::SendError)?;
            Ok(len)
        }
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

fn to_datarate(spreading_factor: SpreadingFactor) -> region::DR {
    match spreading_factor {
        SpreadingFactor::SF7 => region::DR::_5,
        SpreadingFactor::SF8 => region::DR::_4,
        SpreadingFactor::SF9 => region::DR::_3,
        SpreadingFactor::SF10 => region::DR::_2,
        SpreadingFactor::SF11 => region::DR::_1,
        SpreadingFactor::SF12 => region::DR::_0,
    }
}

fn to_lorajoinmode(join_mode: JoinMode) -> LoraJoinMode {
    match join_mode {
        JoinMode::OTAA {
            dev_eui,
            app_eui,
            app_key,
        } => LoraJoinMode::OTAA {
            deveui: dev_eui.reverse().into(),
            appeui: app_eui.reverse().into(),
            appkey: app_key.into(),
        },
        JoinMode::ABP {
            news_key,
            apps_key,
            dev_addr,
        } => LoraJoinMode::ABP {
            newskey: news_key.0.into(),
            appskey: apps_key.0.into(),
            devaddr: LDevAddr::new(dev_addr.0).unwrap(),
        },
    }
}
