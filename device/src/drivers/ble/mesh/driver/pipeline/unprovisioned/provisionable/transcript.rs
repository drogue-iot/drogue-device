use crate::drivers::ble::mesh::crypto::s1;
use crate::drivers::ble::mesh::provisioning::{Capabilities, Invite, PublicKey, Start};
use crate::drivers::ble::mesh::InsufficientBuffer;
use aes::Aes128;
use cmac::crypto_mac::{InvalidKeyLength, Output};
use cmac::Cmac;
use heapless::Vec;

pub struct Transcript {
    confirmation_inputs: Vec<u8, 256>,
}

impl Default for Transcript {
    fn default() -> Self {
        Self::new()
    }
}

impl Transcript {
    pub fn new() -> Self {
        Self {
            confirmation_inputs: Vec::new(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.confirmation_inputs.truncate(0);
    }

    pub(crate) fn add_invite(&mut self, invite: &Invite) -> Result<(), InsufficientBuffer> {
        let mut vec: Vec<u8, 2> = Vec::new();
        invite.emit(&mut vec)?;
        self.confirmation_inputs
            .extend_from_slice(&vec.as_slice()[1..])
            .map_err(|_| InsufficientBuffer)
    }

    pub(crate) fn add_capabilities(
        &mut self,
        capabilities: &Capabilities,
    ) -> Result<(), InsufficientBuffer> {
        let mut vec: Vec<u8, 32> = Vec::new();
        capabilities.emit(&mut vec)?;
        self.confirmation_inputs
            .extend_from_slice(&vec.as_slice()[1..])
            .map_err(|_| InsufficientBuffer)
    }

    pub(crate) fn add_start(&mut self, start: &Start) -> Result<(), InsufficientBuffer> {
        let mut vec: Vec<u8, 32> = Vec::new();
        start.emit(&mut vec)?;
        self.confirmation_inputs
            .extend_from_slice(&vec.as_slice()[1..])
            .map_err(|_| InsufficientBuffer)
    }

    pub(crate) fn add_pubkey_provisioner(
        &mut self,
        pk: &PublicKey,
    ) -> Result<(), InsufficientBuffer> {
        let mut vec: Vec<u8, 65> = Vec::new();
        pk.emit(&mut vec)?;
        self.confirmation_inputs
            .extend_from_slice(&vec.as_slice()[1..])
            .map_err(|_| InsufficientBuffer)
    }

    pub(crate) fn add_pubkey_device(&mut self, pk: &PublicKey) -> Result<(), InsufficientBuffer> {
        let mut vec: Vec<u8, 65> = Vec::new();
        pk.emit(&mut vec)?;
        self.confirmation_inputs
            .extend_from_slice(&vec.as_slice()[1..])
            .map_err(|_| InsufficientBuffer)
    }

    fn confirmation_inputs(&self) -> &[u8] {
        self.confirmation_inputs.as_slice()
    }

    pub(crate) fn confirmation_salt(&self) -> Result<Output<Cmac<Aes128>>, InvalidKeyLength> {
        s1(self.confirmation_inputs())
    }
}
