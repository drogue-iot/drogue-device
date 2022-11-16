use core::future::Future;
use embedded_hal::digital::v2::InputPin;
use embedded_hal_async::digital::Wait;

pub trait Button {
    type WaitPressed<'m>: Future<Output = ()>
    where
        Self: 'm;

    fn wait_pressed<'m>(&'m mut self) -> Self::WaitPressed<'m>
    where
        Self: 'm;

    type WaitReleased<'m>: Future<Output = ()>
    where
        Self: 'm;

    fn wait_released<'m>(&'m mut self) -> Self::WaitReleased<'m>
    where
        Self: 'm;

    type WaitAny<'m>: Future<Output = Event>
    where
        Self: 'm;

    fn wait_any<'m>(&'m mut self) -> Self::WaitAny<'m>
    where
        Self: 'm;
}

impl<P: InputPin + Wait> Button for P {
    type WaitPressed<'m> = impl Future<Output = ()> + 'm where Self: 'm;

    fn wait_pressed<'m>(&'m mut self) -> Self::WaitPressed<'m>
    where
        Self: 'm,
    {
        async move {
            loop {
                self.wait_for_any_edge().await.unwrap();
                if self.is_low().ok().unwrap() {
                    break;
                }
            }
        }
    }

    type WaitReleased<'m> = impl Future<Output = ()> + 'm where Self: 'm;

    fn wait_released<'m>(&'m mut self) -> Self::WaitReleased<'m>
    where
        Self: 'm,
    {
        async move {
            loop {
                self.wait_for_any_edge().await.unwrap();
                if self.is_high().ok().unwrap() {
                    break;
                }
            }
        }
    }

    type WaitAny<'m> = impl Future<Output = Event> + 'm where Self: 'm;

    fn wait_any<'m>(&'m mut self) -> Self::WaitAny<'m>
    where
        Self: 'm,
    {
        async move {
            loop {
                self.wait_for_any_edge().await.unwrap();
                if self.is_low().ok().unwrap() {
                    return Event::Released;
                } else {
                    return Event::Pressed;
                }
            }
        }
    }
}
