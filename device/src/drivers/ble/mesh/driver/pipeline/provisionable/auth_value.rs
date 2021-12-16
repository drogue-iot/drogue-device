use crate::drivers::ble::mesh::driver::pipeline::provisionable::ProvisionableContext;
use crate::drivers::ble::mesh::driver::DeviceError;
use crate::drivers::ble::mesh::provisioning::{
    InputOOBAction, OOBAction, OOBSize, OutputOOBAction, Start,
};
use heapless::Vec;

pub enum AuthValue {
    None,
    InputEvents(u32),
    OutputEvents(u32),
    InputNumeric(u32),
    OutputNumeric(u32),
    InputAlphanumeric(Vec<u8, 8>),
    OutputAlphanumeric(Vec<u8, 8>),
}

impl AuthValue {
    pub fn get_bytes(&self) -> [u8; 16] {
        let mut bytes = [0; 16];
        match self {
            AuthValue::None => {
                // all zeros
            }
            AuthValue::InputEvents(num)
            | AuthValue::OutputEvents(num)
            | AuthValue::InputNumeric(num)
            | AuthValue::OutputNumeric(num) => {
                let num_bytes = num.to_be_bytes();
                bytes[12] = num_bytes[0];
                bytes[13] = num_bytes[1];
                bytes[14] = num_bytes[2];
                bytes[15] = num_bytes[3];
            }
            AuthValue::InputAlphanumeric(chars) | AuthValue::OutputAlphanumeric(chars) => {
                for (i, byte) in chars.iter().enumerate() {
                    bytes[i] = *byte
                }
            }
        }

        bytes
    }
}

pub fn determine_auth_value<C: ProvisionableContext>(
    ctx: &C,
    start: &Start,
) -> Result<AuthValue, DeviceError> {
    Ok(
        match (&start.authentication_action, &start.authentication_size) {
            (
                OOBAction::Output(OutputOOBAction::Blink)
                | OOBAction::Output(OutputOOBAction::Beep)
                | OOBAction::Output(OutputOOBAction::Vibrate),
                OOBSize::MaximumSize(size),
            ) => {
                let auth_raw = random_physical_oob(ctx, *size);
                AuthValue::OutputEvents(auth_raw)
            }
            (
                OOBAction::Input(InputOOBAction::Push) | OOBAction::Input(InputOOBAction::Twist),
                OOBSize::MaximumSize(size),
            ) => {
                let auth_raw = random_physical_oob(ctx, *size);
                AuthValue::InputEvents(auth_raw)
            }
            (OOBAction::Output(OutputOOBAction::OutputNumeric), OOBSize::MaximumSize(size)) => {
                let auth_raw = random_numeric(ctx, *size);
                AuthValue::OutputNumeric(auth_raw)
            }
            // TODO actually dispatch to device/app/thing's UI for inputs instead of just making up shit.
            (OOBAction::Input(InputOOBAction::InputNumeric), OOBSize::MaximumSize(size)) => {
                let auth_raw = random_numeric(ctx, *size);
                AuthValue::InputNumeric(auth_raw)
            }
            (
                OOBAction::Output(OutputOOBAction::OutputAlphanumeric),
                OOBSize::MaximumSize(size),
            ) => {
                let auth_raw = random_alphanumeric(ctx, *size)?;
                AuthValue::OutputAlphanumeric(auth_raw)
            }
            (OOBAction::Input(InputOOBAction::InputAlphanumeric), OOBSize::MaximumSize(size)) => {
                let auth_raw = random_alphanumeric(ctx, *size)?;
                AuthValue::InputAlphanumeric(auth_raw)
            }
            _ => {
                // zeros!
                AuthValue::None
            }
        },
    )
}

fn random_physical_oob<C: ProvisionableContext>(ctx: &C, size: u8) -> u32 {
    // "select a random integer between 0 and 10 to the power of the Authentication Size exclusive"
    //
    // ... which could be an absolute metric tonne of beeps/twists/pushes if AuthSize is large-ish.
    let mut max = 1;
    for _ in 0..size {
        max = max * 10;
    }

    loop {
        let candidate = ctx.rng_u32();
        if candidate > 0 && candidate < max {
            return candidate;
        }
    }
}

fn random_numeric<C: ProvisionableContext>(ctx: &C, size: u8) -> u32 {
    loop {
        let candidate = ctx.rng_u32();

        match size {
            1 => {
                if candidate < 10 {
                    return candidate;
                }
            }
            2 => {
                if candidate < 100 {
                    return candidate;
                }
            }
            3 => {
                if candidate < 1_000 {
                    return candidate;
                }
            }
            4 => {
                if candidate < 10_000 {
                    return candidate;
                }
            }
            5 => {
                if candidate < 100_000 {
                    return candidate;
                }
            }
            6 => {
                if candidate < 1_000_000 {
                    return candidate;
                }
            }
            7 => {
                if candidate < 10_000_000 {
                    return candidate;
                }
            }
            8 => {
                if candidate < 100_000_000 {
                    return candidate;
                }
            }
            _ => {
                // should never get here, but...
                return 0;
            }
        }
    }
}

fn random_alphanumeric<C: ProvisionableContext>(
    ctx: &C,
    size: u8,
) -> Result<Vec<u8, 8>, DeviceError> {
    let mut random = Vec::new();
    for _ in 0..size {
        loop {
            let candidate = ctx.rng_u8();
            if candidate >= 64 && candidate <= 90 {
                // Capital ASCII letters A-Z
                random
                    .push(candidate)
                    .map_err(|_| DeviceError::InsufficientBuffer)?;
            } else if candidate >= 48 && candidate <= 57 {
                // ASCII numbers 0-9
                random
                    .push(candidate)
                    .map_err(|_| DeviceError::InsufficientBuffer)?;
            }
        }
    }
    Ok(random)
}
