//! Board Support Packages (BSP).

pub mod boards;

/// A board capable of creating itself using peripherals.
pub trait Board: Sized {
    type Peripherals;
    type BoardConfig: Default;

    fn new(peripherals: Self::Peripherals) -> Self;

    fn new_with_config(peripherals: Self::Peripherals, _config: Self::BoardConfig) -> Self {
        Self::new(peripherals)
    }
}

#[macro_export]
macro_rules! bind_bsp {
    ($bsp:ty, $app_bsp:ident) => {
        struct $app_bsp($bsp);
        impl $crate::bsp::Board for BSP {
            type Peripherals = <$bsp as $crate::bsp::Board>::Peripherals;
            type BoardConfig = <$bsp as $crate::bsp::Board>::BoardConfig;

            fn new(peripherals: Self::Peripherals) -> Self {
                BSP(<$bsp>::new(peripherals))
            }

            fn new_with_config(peripherals: Self::Peripherals, config: Self::BoardConfig) -> Self {
                BSP(<$bsp>::new_with_config(peripherals, config))
            }
        }
    };
}
