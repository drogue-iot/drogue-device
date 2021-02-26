#![no_std]

pub mod adapter;
mod buffer;
pub mod ingress;
pub mod network;
mod num;
mod parser;
pub mod protocol;

pub use adapter::initialize;

#[cfg(all(not(feature="1k"),not(feature="2k")))]
pub const BUFFER_LEN: usize = 512;
#[cfg(all(feature="1k",not(feature="2k")))]
pub const BUFFER_LEN: usize = 1024;
#[cfg(all(feature="2k",not(feature="1k")))]
pub const BUFFER_LEN: usize = 2048;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
