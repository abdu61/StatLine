//! Shared identity, model types, units and formatting helpers.
//!
//! This crate is pure logic: no Win32, no I/O, no globals. That keeps it
//! trivially unit-testable and safe to depend on from every other crate.

pub mod model;
pub mod ring;
pub mod units;

pub use model::{
    AdapterSample, BatterySample, CpuSample, DiskSample, GpuSample, MemorySample, NetworkSample,
    Snapshot, SystemSample, ThermalSample, VolumeSample,
};
pub use ring::RingBuffer;

/// Product name. Used in window titles, tray tooltips and log headers.
pub const APP_NAME: &str = "StatLine";

/// Stable identifier for mutexes, message names and package identity.
pub const APP_ID: &str = "StatLine";

/// Semantic version of the running binary.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_set() {
        assert_eq!(APP_NAME, "StatLine");
        assert!(!VERSION.is_empty());
    }
}
