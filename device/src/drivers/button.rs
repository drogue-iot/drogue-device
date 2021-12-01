use crate::traits::button::Event;
use core::future::Future;
use embassy::traits::gpio::WaitForAnyEdge;
use embedded_hal::digital::v2::InputPin;

pub struct Button<P: WaitForAnyEdge + InputPin + 'static> {
    pin: P,
}

impl<P: WaitForAnyEdge + InputPin + 'static> Button<P> {
    pub fn new(pin: P) -> Self {
        Self { pin }
    }
}

impl<P: WaitForAnyEdge + InputPin + 'static> crate::traits::button::Button for Button<P> {
    type WaitPressed<'m>
    where
        Self: 'm,
    = impl Future<Output = ()> + 'm;

    fn wait_pressed<'m>(&'m mut self) -> Self::WaitPressed<'m>
    where
        Self: 'm,
    {
        async move {
            loop {
                self.pin.wait_for_any_edge().await;
                if self.pin.is_low().ok().unwrap() {
                    break;
                };
            }
        }
    }

    type WaitReleased<'m>
    where
        Self: 'm,
    = impl Future<Output = ()> + 'm;

    fn wait_released<'m>(&'m mut self) -> Self::WaitReleased<'m>
    where
        Self: 'm,
    {
        async move {
            loop {
                self.pin.wait_for_any_edge().await;
                if self.pin.is_high().ok().unwrap() {
                    break;
                };
            }
        }
    }

    type WaitAny<'m>
    where
        Self: 'm,
    = impl Future<Output = Event> + 'm;

    fn wait_any<'m>(&'m mut self) -> Self::WaitAny<'m>
    where
        Self: 'm,
    {
        async move {
            loop {
                self.pin.wait_for_any_edge().await;
                if self.pin.is_high().ok().unwrap() {
                    return Event::Released;
                } else {
                    return Event::Pressed;
                }
            }
        }
    }
}
