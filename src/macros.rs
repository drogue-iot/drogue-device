

#[macro_export]
macro_rules! device {

    ($ty:ty = $device:expr; $memory:literal  ) => {
        static mut DEVICE: Option<$crate::device::DeviceContext<$ty>> = None;
        let device = unsafe {
            DEVICE.replace( $crate::device::DeviceContext::new( $device ) );
            DEVICE.as_mut().unwrap()
        };

        $crate::init_heap!($memory);

        device.start();

        #[exception]
        fn DefaultHandler(irqn: i16) {
            unsafe {
                DEVICE.as_ref().unwrap().on_interrupt(irqn);
            }
        }
    }

}