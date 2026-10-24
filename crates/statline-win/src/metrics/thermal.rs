//! ACPI thermal zone temperature via PDH.
//!
//! The Windows counter is in whole degrees Kelvin (the ACPI `_TMP` value in
//! tenths of a Kelvin is converted by the PDH bridge). Values outside a sane
//! device range are discarded, so a provider that reports something else hides
//! instead of showing a fabricated temperature.

use crate::metrics::pdh::{PdhCounter, PdhQuery};

const ABSOLUTE_ZERO_C: f64 = 273.15;

/// Celsius range a thermal zone can plausibly report.
const MIN_C: f32 = -20.0;
const MAX_C: f32 = 125.0;

/// One cached PDH query over every ACPI thermal zone.
pub struct ThermalCounters {
    query: PdhQuery,
    temperature: PdhCounter,
}

impl ThermalCounters {
    pub fn open() -> Option<Self> {
        let query = PdhQuery::open()?;
        let temperature = query.add_english("\\Thermal Zone Information(*)\\Temperature")?;
        // Prime the counter; a raw count has data immediately, but this keeps
        // the first sample consistent with the rate counters.
        let _ = query.collect();
        Some(Self { query, temperature })
    }

    /// Hottest valid zone in °C. Multiple zones exist on most laptops.
    pub fn sample(&self) -> Option<f32> {
        if !self.query.collect() {
            return None;
        }
        let values = self.temperature.values()?;
        values.into_iter().filter_map(to_celsius).reduce(f32::max)
    }
}

fn to_celsius(kelvin: f64) -> Option<f32> {
    let celsius = (kelvin - ABSOLUTE_ZERO_C) as f32;
    (MIN_C..=MAX_C).contains(&celsius).then_some(celsius)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kelvin_is_converted_and_bounded() {
        assert!((to_celsius(350.0).expect("plausible") - 76.85).abs() < 0.01);
        assert_eq!(to_celsius(0.0), None);
        assert_eq!(to_celsius(2700.0), None);
    }

    #[test]
    fn sampling_is_optional_but_plausible_when_present() {
        if let Some(counters) = ThermalCounters::open() {
            if let Some(celsius) = counters.sample() {
                assert!((MIN_C..=MAX_C).contains(&celsius));
            }
        }
    }
}
