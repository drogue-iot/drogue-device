use crate::domain::{
    temperature::{Celsius, Temperature},
    SensorAcquisition,
};
use crate::drivers::sensors::hts221::*;
use crate::traits::sensors::temperature::*;

use crate::{Actor, Address, Inbox};
use core::future::Future;
use core::marker::PhantomData;
use embassy::traits::gpio::WaitForAnyEdge;
use embassy::traits::i2c::*;
use embedded_hal::digital::v2::InputPin;

pub struct Sensor<P, I>
where
    P: WaitForAnyEdge + InputPin + 'static,
    I: I2c<SevenBitAddress> + 'static,
{
    hts221: Hts221,
    _data: PhantomData<&'static I>,
    ready: P,
}

impl<P, I> Sensor<P, I>
where
    P: WaitForAnyEdge + InputPin + 'static,
    I: I2c<SevenBitAddress> + 'static,
{
    pub fn new(ready: P) -> Self {
        Self {
            hts221: Hts221::new(),
            _data: PhantomData,
            ready,
        }
    }

    async fn wait_ready(&mut self) {
        while !self.ready.is_high().ok().unwrap() {
            self.ready.wait_for_any_edge().await;
        }
    }
}

pub struct ReadTemperature;

impl<P, I> Actor for Sensor<P, I>
where
    P: WaitForAnyEdge + InputPin + 'static,
    I: I2c<SevenBitAddress> + 'static,
    <I as I2c>::Error: Send,
{
    type Message<'m> = ReadTemperature;
    type Response = Option<Result<SensorAcquisition<Celsius>, Hts221Error<I::Error>>>;

    type Configuration = I;

    type OnMountFuture<'m, M>
    where
        Self: 'm,
        M: 'm,
        I: 'm,
    = impl Future<Output = ()> + 'm;

    fn on_mount<'m, M>(
        &'m mut self,
        mut i2c: Self::Configuration,
        _: Address<'static, Self>,
        inbox: &'m mut M,
    ) -> Self::OnMountFuture<'m, M>
    where
        M: Inbox<'m, Self> + 'm,
    {
        async move {
            self.hts221.initialize(&mut i2c).await.ok();
            loop {
                if let Some(mut m) = inbox.next().await {
                    self.wait_ready().await;
                    let data = self.hts221.read(&mut i2c).await;
                    m.set_response(Some(data));
                }
            }
        }
    }
}

impl<P, I> TemperatureSensor<Celsius> for Address<'static, Sensor<P, I>>
where
    P: WaitForAnyEdge + InputPin + 'static,
    I: I2c<SevenBitAddress> + 'static,
    <I as I2c>::Error: Send,
{
    type Error = Hts221Error<<I as I2c>::Error>;

    type ReadFuture<'m>
    where
        P: 'm,
        I: 'm,
    = impl Future<Output = Result<Temperature<Celsius>, Self::Error>> + 'm;

    fn temperature<'m>(&'m mut self) -> Self::ReadFuture<'m> {
        async move {
            self.request(ReadTemperature)
                .unwrap()
                .await
                .unwrap()
                .map(|s| s.temperature)
        }
    }
}
