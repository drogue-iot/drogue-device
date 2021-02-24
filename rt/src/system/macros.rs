//! Macros

/// Macro to start a system.
///
/// It takes the type of your device which implements `Device` along with
/// an expression pointing to your device instance.
///
/// Additionally, the size of the async pool, in bytes, should be provided.
///
/// Usage:
/// ```ignore
/// use drogue_device::prelude::*;
///
/// struct MyDevice {}
/// let instance = MyDevice {};
/// impl Device for MyDevice {
///     fn mount(&'static mut self, _: &Address<EventBus<Self>>, _: &mut Supervisor) {}
/// }
///
/// device!( MyDevice = instance; 1024 );
/// ```
#[macro_export]
macro_rules! device {
    ($ty:ty = $device:expr; $memory:literal  ) => {
        static mut DEVICE: Option<$crate::system::DeviceContext<$ty>> = None;
        let device = unsafe {
            DEVICE.replace($crate::system::DeviceContext::new($device));
            DEVICE.as_mut().unwrap()
        };

        $crate::init_arena!($crate::system| SystemArena => $memory);

        device.mount();

        #[exception]
        fn DefaultHandler(irqn: i16) {
            unsafe {
                DEVICE.as_ref().unwrap().on_interrupt(irqn);
            }
        }
    };
}
