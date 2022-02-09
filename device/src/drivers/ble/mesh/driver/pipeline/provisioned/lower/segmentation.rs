use crate::drivers::ble::mesh::address::{Address, UnicastAddress};
use crate::drivers::ble::mesh::app::ApplicationKeyIdentifier;
use crate::drivers::ble::mesh::configuration_manager::NetworkKeyHandle;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::pdu::lower::SzMic;
use crate::drivers::ble::mesh::pdu::upper::UpperPDU;
use heapless::Vec;
use crate::drivers::ble::mesh::InsufficientBuffer;

pub struct Segmentation {
    in_flight: [Option<InFlight>; 3],
}

impl Default for Segmentation {
    fn default() -> Self {
        Self {
            in_flight: Default::default(),
        }
    }
}

impl Segmentation {
    pub(crate) fn process_inbound(
        &mut self,
        src: UnicastAddress,
        seq_zero: u16,
        seg_o: u8,
        seg_n: u8,
        segment_m: &Vec<u8, 12>,
    ) -> Result<Option<Vec<u8, 380>>, DeviceError> {
        defmt::info!("reassemble {} {} {}", src, seq_zero, seg_o);
        defmt::info!("--> {:x}", segment_m);
        let in_flight_index = self.find_or_create_in_flight(
            src,
            seq_zero,
            seg_n,
        )?;

        if let Some(in_flight) = &mut self.in_flight[in_flight_index] {
            if let Some( all) = in_flight.process_inbound(seg_o, segment_m)? {
                self.in_flight[in_flight_index] = None;
                defmt::info!("fully assembled");
                Ok(Some(all))
            } else {
                Ok(None)
            }
        } else {
            Err(DeviceError::InsufficientBuffer)
        }
    }

    fn find_or_create_in_flight(
        &mut self,
        src: UnicastAddress,
        seq_zero: u16,
        seg_n: u8,
    ) -> Result<usize, InsufficientBuffer> {
        if let Some((index, _)) = self.in_flight.iter_mut().enumerate().find(|(_, e)| {
            if let Some(e) = e {
                e.src == src && e.seq_zero == seq_zero && e.seg_n == seg_n
            } else {
                false
            }
        } ) {
            Ok(index)
        } else {
            if let Some((index, slot)) = self.in_flight.iter_mut().enumerate().find(|(_, e)| matches!(e, None)) {
                let in_flight = InFlight::new(
                    src,
                    seq_zero,
                    seg_n,
                );
                self.in_flight[index] = Some(in_flight);
                Ok(index)
            } else {
                Err(InsufficientBuffer)
            }
        }
    }
}

struct InFlight {
    src: UnicastAddress,
    seq_zero: u16,
    seg_n: u8,
    segments: Vec<Option<Vec<u8, 12>>, 32>,
}

impl InFlight {
    fn new(
        src: UnicastAddress,
        seq_zero: u16,
        seg_n: u8,
    ) -> Self {
        let mut segments = Vec::new();
        for _ in 0..=seg_n {
            segments.push(None);
        }
        Self {
            src,
            seq_zero,
            seg_n,
            segments,
        }
    }

    fn process_inbound(&mut self, seg_n: u8, segment_m: &Vec<u8, 12>) -> Result<Option<Vec<u8, 380>>, InsufficientBuffer> {
        if matches!(self.segments[seg_n as usize], None) {
            let mut inner = Vec::new();
            inner.extend_from_slice(segment_m).map_err(|_|InsufficientBuffer)?;
            self.segments[seg_n as usize] = Some(inner);
            if self.segments.iter().all(|e| !matches!(e, None)) {
                let mut all = Vec::new();
                for segment in self.segments.iter() {
                    if let Some(segment) = segment {
                        all.extend_from_slice(segment).map_err(|_| InsufficientBuffer)?
                    }
                }
                Ok(Some(all))
            } else {
                Ok(None)
            }
        } else {
            Ok(None)
        }
    }
}
