#![macro_use]
#![allow(unused_macros)]

macro_rules! panic {
    ($($x:tt)*) => {
        {
            ::defmt::panic!($($x)*);
        }
    };
}

macro_rules! trace {
    ($s:literal $(, $x:expr)* $(,)?) => {
        {
            ::log::trace!($s $(, $x)*);
            ::defmt::trace!($s $(, $x)*);
        }
    };
}

macro_rules! debug {
    ($s:literal $(, $x:expr)* $(,)?) => {
        {
            ::log::debug!($s $(, $x)*);
            ::defmt::debug!($s $(, $x)*);
        }
    };
}

macro_rules! info {
    ($s:literal $(, $x:expr)* $(,)?) => {
        {
            ::log::info!($s $(, $x)*);
            ::defmt::info!($s $(, $x)*);
        }
    };
}

macro_rules! warn {
    ($s:literal $(, $x:expr)* $(,)?) => {
        {
            ::log::warn!($s $(, $x)*);
            ::defmt::warn!($s $(, $x)*);
        }
    };
}

macro_rules! error {
    ($s:literal $(, $x:expr)* $(,)?) => {
        {
            ::log::error!($s $(, $x)*);
            ::defmt::error!($s $(, $x)*);
        }
    };
}

macro_rules! unwrap {
    ($($x:tt)*) => {
        ::defmt::unwrap!($($x)*)
    };
}
