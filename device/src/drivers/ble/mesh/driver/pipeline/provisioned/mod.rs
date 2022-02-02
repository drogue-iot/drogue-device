use crate::drivers::ble::mesh::driver::pipeline::provisioned::access::AccessContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::lower::LowerContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::authentication::AuthenticationContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::network::relay::RelayContext;
use crate::drivers::ble::mesh::driver::pipeline::provisioned::upper::UpperContext;

pub mod lower;
pub mod network;
pub mod upper;
pub mod access;

pub trait ProvisionedContext:
    AuthenticationContext + RelayContext + LowerContext + UpperContext + AccessContext
{
}
