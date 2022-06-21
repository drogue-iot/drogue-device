use crate::traits::button::Event;
use core::future::Future;
use core::marker::PhantomData;
use embedded_hal::digital::v2::InputPin;
use embedded_hal_async::digital::Wait;

pub use crate::drivers::ActiveHigh;
pub use crate::drivers::ActiveLow;

pub trait Active {
    fn is_pressed<P: InputPin>(pin: &P) -> Result<bool, P::Error>;
    fn is_released<P: InputPin>(pin: &P) -> Result<bool, P::Error>;
}

impl Active for ActiveHigh {
    fn is_pressed<P: InputPin>(pin: &P) -> Result<bool, P::Error> {
        pin.is_high()
    }

    fn is_released<P: InputPin>(pin: &P) -> Result<bool, P::Error> {
        pin.is_low()
    }
}

impl Active for ActiveLow {
    fn is_pressed<P: InputPin>(pin: &P) -> Result<bool, P::Error> {
        pin.is_low()
    }

    fn is_released<P: InputPin>(pin: &P) -> Result<bool, P::Error> {
        pin.is_high()
    }
}

pub struct Button<P, ACTIVE = ActiveLow>
where
    P: Wait + InputPin + 'static,
    ACTIVE: Active,
{
    pin: P,
    _marker: PhantomData<ACTIVE>,
}

impl<P, ACTIVE> Button<P, ACTIVE>
where
    P: Wait + InputPin + 'static,
    ACTIVE: Active,
{
    pub fn new(pin: P) -> Self {
        Self {
            pin,
            _marker: PhantomData,
        }
    }
}

impl<P, ACTIVE> Button<P, ACTIVE>
where
    P: Wait + InputPin + 'static,
    ACTIVE: Active,
{
    pub async fn wait_pressed(&mut self) {
        loop {
            self.pin.wait_for_any_edge().await.unwrap();
            if ACTIVE::is_pressed(&self.pin).unwrap_or(false) {
                break;
            };
        }
    }

    pub async fn wait_released(&mut self) {
        loop {
            self.pin.wait_for_any_edge().await.unwrap();
            if ACTIVE::is_released(&self.pin).unwrap_or(false) {
                break;
            };
        }
    }

    pub async fn wait_any(&mut self) -> Event {
        loop {
            self.pin.wait_for_any_edge().await.unwrap();
            if ACTIVE::is_pressed(&self.pin).unwrap_or(false) {
                return Event::Pressed;
            }
            if ACTIVE::is_released(&self.pin).unwrap_or(false) {
                return Event::Released;
            }
        }
    }
}

impl<P, ACTIVE> crate::traits::button::Button for Button<P, ACTIVE>
where
    P: Wait + InputPin + 'static,
    ACTIVE: Active,
{
    type WaitPressed<'m> = impl Future<Output = ()> + 'm where Self: 'm;

    fn wait_pressed<'m>(&'m mut self) -> Self::WaitPressed<'m>
    where
        Self: 'm,
    {
        Button::wait_pressed(self)
    }

    type WaitReleased<'m> = impl Future<Output = ()> + 'm where Self: 'm;

    fn wait_released<'m>(&'m mut self) -> Self::WaitReleased<'m>
    where
        Self: 'm,
    {
        Button::wait_released(self)
    }

    type WaitAny<'m> = impl Future<Output = Event> + 'm where Self: 'm;

    fn wait_any<'m>(&'m mut self) -> Self::WaitAny<'m>
    where
        Self: 'm,
    {
        Button::wait_any(self)
    }
}
