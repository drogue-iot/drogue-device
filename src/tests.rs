use crate::device::{Device, DeviceContext};
use crate::actor::{ActorContext, Actor};
use crate::handler::{RequestHandler, NotificationHandler, Response, Completion};
use crate::supervisor::Supervisor;

use crate::init_heap;
use crate::interrupt::{Interrupt, InterruptContext};


struct MyDevice {
    led: ActorContext<LED>,
    button: InterruptContext<Button>,
}

impl Device for MyDevice {
    fn start(&'static mut self, supervisor: &mut Supervisor) {
        let led_addr = self.led.start(supervisor);
        led_addr.notify(LEDState::On);
        led_addr.request(LEDState::Off);
    }
}

enum LEDState {
    On,
    Off,
}

struct LED {}

impl LED {
    pub fn turn_on(&mut self) {}

    pub fn turn_off(&mut self) {}
}

impl Actor for LED {}

impl RequestHandler<LEDState> for LED {
    type Response = u8;

    fn on_request(&'static mut self, message: LEDState) -> Response<Self::Response>
    {
        Response::defer(async move {
            self.turn_off();
            42
        })
    }
}

impl NotificationHandler<LEDState> for LED {
    fn on_notification(&'static mut self, message: LEDState) -> Completion {
        Completion::defer(async move {
            self.turn_off()
        })
    }
}

struct Button {

}

impl Interrupt for Button {
    fn irq(&self) -> u8 {
        unimplemented!()
    }

    fn on_interrupt(&self) {
        unimplemented!()
    }
}

#[test]
fn the_api() {
    init_heap!( 1024 );

    static mut DEVICE: Option<DeviceContext<MyDevice>> = None;
    println!("A");
    let led = LED {};
    let mut device = MyDevice {
        led: ActorContext::new(led),
        button: InterruptContext::new( Button {} ),
    };
    println!("B");

    let device = unsafe {
        DEVICE.replace(DeviceContext::new(device));
        DEVICE.as_mut().unwrap()
    };
    println!("C");

    let mut supervisor = Supervisor::new();
    println!("D");

    device.start(&mut supervisor);
}
