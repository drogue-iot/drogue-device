//#![no_std]
mod handler;
mod actor;
mod address;
mod device;
mod supervisor;

#[cfg(test)]
mod tests {
    use crate::device::{Device, DeviceContext};
    use crate::actor::{ActorContext, Actor};
    use crate::handler::{AskHandler, TellHandler, Response, Completion};
    use core::pin::Pin;
    use std::future::Future;
    use crate::supervisor::alloc;
    use crate::supervisor::Box;
    use heapless::consts::*;

    struct MyDevice {
        led: ActorContext<LED>,
        button: Button,
    }

    impl Device for MyDevice {
        fn start(&'static self) {
            let led_addr = self.led.start();
            led_addr.tell(LEDState::On);
            led_addr.ask( LEDState::Off );
            //self.button.start();
        }
    }

    enum LEDState {
        On,
        Off,
    }

    struct LED {

    }

    impl LED {
        pub fn turn_on(&mut self) {

        }

        pub fn turn_off(&mut self) {

        }
    }

    impl Actor for LED {
    }

    impl AskHandler<LEDState> for LED {
        type Response = u8;

        fn on_message(&'static mut self, message: LEDState) -> Response<Self::Response>
        {
            Response::defer( async move {
                self.turn_off();
                42
            })
        }
    }

    impl TellHandler<LEDState> for LED {
        fn on_message(&'static mut self, message: LEDState) -> Completion {
            Completion::defer( async move {
                self.turn_off()
            })
        }
    }

    struct Button {

    }

    #[test]
    fn the_api() {
        static mut DEVICE: Option<DeviceContext<MyDevice>> = None;
        let led = LED{};
        let device = MyDevice {
            led: ActorContext::new( led ),
            button: Button{}
        };

        let device = unsafe {
            DEVICE.replace( DeviceContext::new( device ) );
            DEVICE.as_ref().unwrap()
        };

        device.start();
    }
}

