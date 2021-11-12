use embassy::traits::gpio::WaitForAnyEdge;
use embedded_hal::digital::v2::InputPin;

pub struct Button<P: WaitForAnyEdge + InputPin + 'static> {
    pin: P,
}

impl<P: WaitForAnyEdge + InputPin + 'static> Button<P> {
    pub fn new(pin: P) -> Self {
        Self { pin }
    }

    pub async fn wait_pressed(&mut self) {
        loop {
            self.pin.wait_for_any_edge().await;
            if self.pin.is_low().ok().unwrap() {
                break;
            };
        }
    }

    pub async fn wait_released(&mut self) {
        loop {
            self.pin.wait_for_any_edge().await;
            if self.pin.is_high().ok().unwrap() {
                break;
            };
        }
    }
}
