use core::cmp;
use rubble::att::{AttUuid, Attribute, AttributeProvider, Handle, HandleRange};
use rubble::uuid::Uuid16;
use rubble::Error;

#[derive(Debug)]
pub enum Value {
    ServiceDef([u8; 2]),
    CharDef([u8; 5]),
    CharValue([u8; 4]),
}

/// An `AttributeProvider` that will enumerate as a Environmental Sensing Service.
pub struct EnvironmentSensingService {
    attributes: [Attribute<Value>; 3],
}

const PRIMARY_SERVICE_UUID: Uuid16 = Uuid16(0x2800);
pub const ESS_UUID: Uuid16 = Uuid16(0x2803);
const ESS_TEMPERATURE_MEASUREMENT: Uuid16 = Uuid16(0x2A1C);

impl AsRef<[u8]> for Value {
    fn as_ref(&self) -> &[u8] {
        match self {
            Value::ServiceDef(v) => &v[..],
            Value::CharDef(v) => &v[..],
            Value::CharValue(v) => &v[..],
        }
    }
}

impl EnvironmentSensingService {
    pub fn new() -> Self {
        Self {
            attributes: [
                Attribute::new(
                    AttUuid::Uuid16(PRIMARY_SERVICE_UUID),
                    Handle::from_raw(0x0001),
                    Value::ServiceDef([0x1A, 0x18]),
                ), // "ES Service" = 0x181A
                // Define temperature measurement
                Attribute::new(
                    AttUuid::Uuid16(ESS_UUID),
                    Handle::from_raw(0x0002),
                    Value::CharDef([
                        0x02, // 1 byte properties: READ = 0x02, NOTIFY = 0x10
                        0x03, 0x00, // 2 bytes handle = 0x0003
                        0x1C, 0x2A, // 2 bytes UUID = 0x2A1C (Temperature measurement)
                    ]),
                ),
                // Characteristic value (Temperature measurement)
                Attribute::new(
                    AttUuid::Uuid16(ESS_TEMPERATURE_MEASUREMENT),
                    Handle::from_raw(0x0003),
                    Value::CharValue([0; 4]),
                ),
                /*
                // Define properties
                Attribute {
                    att_type: Uuid16(0x2803).into(), // "Characteristic"
                    handle: Handle::from_raw(0x0005),
                    value: HexSlice(&[
                        0x02, // 1 byte properties: READ = 0x02
                        0x06, 0x00, // 2 bytes handle = 0x0006
                        0x0C, 0x29, // 2 bytes UUID = 0x2A1C (ES measurement)
                    ]),
                },
                // Characteristic
                Attribute {
                    att_type: AttUuid::Uuid16(Uuid16(0x290C)),
                    handle: Handle::from_raw(0x0006),
                    value: HexSlice(&[
                        0x00, 0x00, // Flags
                        0x02, // Sampling function: Arithmetic mean
                        0x00, 0x00, 0x00, // Measurement period
                        0x00, 0x00, 0x00, // Update interval
                        0x01, // Application: Air
                        0x00, // Uncertainty
                    ]),
                },*/
            ],
        }
    }

    pub fn set_temperature(&mut self, value: u32) {
        self.attributes[2].set_value(Value::CharValue([
            (value >> 24) as u8,
            (value >> 16) as u8,
            (value >> 8) as u8,
            value as u8,
        ]));
    }
}

impl AttributeProvider for EnvironmentSensingService {
    fn for_attrs_in_range(
        &mut self,
        range: HandleRange,
        mut f: impl FnMut(&Self, &Attribute<dyn AsRef<[u8]>>) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let count = self.attributes.len();
        let start = usize::from(range.start().as_u16() - 1); // handles start at 1, not 0
        let end = usize::from(range.end().as_u16() - 1);

        // Update temperature before invoking callback

        let attrs = if start >= count {
            &[]
        } else {
            let end = cmp::min(count - 1, end);
            &self.attributes[start..=end]
        };

        for attr in attrs {
            f(self, attr)?;
        }
        Ok(())
    }

    fn is_grouping_attr(&self, uuid: AttUuid) -> bool {
        uuid == Uuid16(0x2800) // FIXME not characteristics?
    }

    fn group_end(&self, handle: Handle) -> Option<&Attribute<dyn AsRef<[u8]>>> {
        match handle.as_u16() {
            0x0001 => Some(&self.attributes[2]),
            0x0002 => Some(&self.attributes[2]),
            _ => None,
        }
    }
}
