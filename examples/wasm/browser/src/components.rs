use core::future::Future;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, Ordering};
use drogue_device::traits::led::Led;
use embassy::traits::gpio::WaitForAnyEdge;
use embassy::util::Signal;
use embedded_hal::digital::v2::OutputPin;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

// Need somewhere to store these 'peripherals' and their state statically
pub static mut INPUT1: InputPin = InputPin::new();
pub static mut OUTPUT1: OutputPin = OutputPin::new();

pub struct WebLed {
    pin: &'static OutputPin,
}

impl WebLed {
    pub fn new(pin: &'static OutputPin) -> Self {
        Self { pin }
    }
}

impl embedded_hal::digital::v2::OutputPin for WebLed {
    type Error = ();
    fn set_high(&mut self) -> Result<(), ()> {
        self.pin.set_value(true);
        Ok(())
    }

    fn set_low(&mut self) -> Result<(), ()> {
        self.pin.set_value(false);
        Ok(())
    }
}

impl Led for WebLed {
    type Error = ();

    fn on(&mut self) -> Result<(), Self::Error> {
        self.set_high();
        Ok(())
    }

    fn off(&mut self) -> Result<(), Self::Error> {
        self.set_low();
        Ok(())
    }
}

pub struct WebButton {
    pin: &'static InputPin,
}

impl WebButton {
    pub fn new(pin: &'static InputPin) -> Self {
        Self { pin }
    }
}

impl embedded_hal::digital::v2::InputPin for WebButton {
    type Error = ();
    fn is_high(&self) -> Result<bool, ()> {
        Ok(self.pin.get_value())
    }
    fn is_low(&self) -> Result<bool, ()> {
        Ok(self.pin.get_value())
    }
}

impl WaitForAnyEdge for WebButton {
    type Future<'a> = SignalFuture<'a, ()>;
    fn wait_for_any_edge<'a>(&'a mut self) -> Self::Future<'a> {
        self.pin.wait()
    }
}

pub struct InputPin {
    high: AtomicBool,
    signal: Signal<()>,
}

impl InputPin {
    pub const fn new() -> Self {
        Self {
            high: AtomicBool::new(true),
            signal: Signal::new(),
        }
    }

    pub fn configure(&'static mut self, element: &str) {
        let window = web_sys::window().expect("no global `window` exists");
        let document = window.document().expect("should have a document on window");

        let cb = Closure::wrap(Box::new(move || {
            if self.high.load(Ordering::Acquire) {
                self.high.store(false, Ordering::Release);
            } else {
                self.high.store(true, Ordering::Release);
            }
            self.signal.signal(());
        }) as Box<dyn FnMut()>);

        let val = document
            .get_element_by_id(element)
            .expect("unable to find element");
        val.add_event_listener_with_callback("click", cb.as_ref().unchecked_ref())
            .expect("error adding event listener");
        cb.forget();
    }

    fn get_value(&self) -> bool {
        self.high.load(Ordering::Acquire)
    }

    fn wait<'a>(&'a self) -> SignalFuture<'a, ()> {
        self.signal.reset();
        SignalFuture::new(&self.signal)
    }
}

pub struct OutputPin {
    high: AtomicBool,
    element: MaybeUninit<&'static str>,
    transform: MaybeUninit<fn(bool) -> &'static str>,
}

impl OutputPin {
    pub const fn new() -> Self {
        Self {
            high: AtomicBool::new(false),
            element: MaybeUninit::uninit(),
            transform: MaybeUninit::uninit(),
        }
    }

    pub fn configure(
        &'static mut self,
        element: &'static str,
        transform: fn(bool) -> &'static str,
    ) {
        unsafe {
            let p = self.element.as_mut_ptr();
            p.write(element);
            let f = self.transform.as_mut_ptr();
            f.write(transform);
        };
    }

    pub fn set_value(&self, high: bool) {
        let (element, transform) = unsafe { (&*self.element.as_ptr(), &*self.transform.as_ptr()) };
        self.high.store(high, Ordering::Release);
        let window = web_sys::window().expect("no global `window` exists");
        let document = window.document().expect("should have a document on window");
        let txt = document.get_element_by_id(element).unwrap();
        let output = transform(high);
        txt.set_inner_html(output);
    }
}

pub struct SignalFuture<'s, T: Send> {
    signal: &'s Signal<T>,
}

impl<'s, T: Send> SignalFuture<'s, T> {
    pub fn new(signal: &'s Signal<T>) -> Self {
        Self { signal }
    }
}

impl<T: Send> Future for SignalFuture<'_, T> {
    type Output = T;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        self.signal.poll_wait(cx)
    }
}
