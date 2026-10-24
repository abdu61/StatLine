//! Battery state and remaining runtime.

use std::mem::size_of;
use windows::Win32::System::Power::{
    CallNtPowerInformation, GetSystemPowerStatus, SYSTEM_BATTERY_STATE, SYSTEM_POWER_STATUS,
    SystemBatteryState,
};

/// Battery reading, or `None` on a machine without one.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BatteryReading {
    pub percent: f32,
    pub on_ac: bool,
    pub seconds_left: Option<u32>,
    /// Charge (+ ) or discharge (âˆ’) rate in milliwatts.
    pub rate_mw: Option<i32>,
}

pub fn battery() -> Option<BatteryReading> {
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: `status` is a valid out-parameter.
    unsafe { GetSystemPowerStatus(&mut status) }.ok()?;

    // BatteryFlag 255 means "no system battery".
    let present = status.BatteryFlag != 255 && status.BatteryLifePercent != 255;
    if !present {
        return None;
    }

    let percent = status.BatteryLifePercent as f32;
    let on_ac = status.ACLineStatus == 1;
    let seconds_left = if status.BatteryLifeTime == u32::MAX || on_ac {
        None
    } else {
        Some(status.BatteryLifeTime)
    };

    Some(BatteryReading {
        percent,
        on_ac,
        seconds_left,
        rate_mw: battery_rate(),
    })
}

fn battery_rate() -> Option<i32> {
    let mut state = SYSTEM_BATTERY_STATE::default();
    let bytes = size_of::<SYSTEM_BATTERY_STATE>() as u32;

    // SAFETY: no input buffer; the output buffer is correctly sized and aligned.
    let status = unsafe {
        CallNtPowerInformation(
            SystemBatteryState,
            None,
            0,
            Some((&mut state as *mut SYSTEM_BATTERY_STATE).cast()),
            bytes,
        )
    };
    if status.0 < 0 || !state.BatteryPresent {
        return None;
    }

    // `Rate` is milliwatts; discharging is negative on most systems.
    Some(state.Rate as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn battery_percent_is_in_range_when_present() {
        if let Some(battery) = battery() {
            assert!((0.0..=100.0).contains(&battery.percent));
        }
    }
}
