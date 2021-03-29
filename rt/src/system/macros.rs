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
    ($ty:ty = $configure:expr; $memory:literal  ) => {
        static mut DEVICE: Option<$crate::system::DeviceContext<$ty>> = None;

        // Make sure device don't end up consuming our stack
        fn initialize() -> &'static $crate::system::DeviceContext<$ty> {
            let d = $configure();
            unsafe {
                DEVICE.replace($crate::system::DeviceContext::new(d));
                DEVICE.as_ref().unwrap()
            }
        }

        let device = initialize();

        //         $crate::arena::init_arena!($crate::system| SystemArena => $memory);

        device.mount();

        #[cfg(target_arch = "arm")]
        #[exception]
        fn DefaultHandler(irqn: i16) {
            unsafe {
                DEVICE.as_ref().unwrap().on_interrupt(irqn);
            }
        }

        device.run();
    };
}
