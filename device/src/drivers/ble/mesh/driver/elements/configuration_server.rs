use crate::drivers::ble::mesh::driver::node::{Node, Receiver, Transmitter};
use crate::drivers::ble::mesh::storage::Storage;
use rand_core::{CryptoRng, RngCore};

/*
impl ConfigurationServerHandler for ConfigurationServerState
{
    type BEACON = BeaconState;

    fn beacon(&self) -> &Self::BEACON {
        &self.configuration_server_state.beacon
    }

    fn beacon_mut(&mut self) -> &mut Self::BEACON {
        &mut self.configuration_server_state.beacon
    }
}

pub struct ConfigurationServerState {
    beacon: BeaconState,
}

impl ConfigurationServerState {
    pub fn new() -> Self {
        Self {
            beacon: BeaconState::new(),
        }
    }
}

pub struct BeaconState {
    val: bool,
}

impl BeaconState {
    pub fn new() -> Self {
        Self {
            val: false,
        }
    }

}

impl BeaconHandler for BeaconState {
    fn set(&mut self, val: bool) {
        self.val = val;
    }

    fn get(&self) -> bool {
        self.val
    }
}
 */
