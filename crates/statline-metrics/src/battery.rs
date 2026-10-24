//! Battery sampling. Returns `None` on desktops, which is expected.

use statline_core::model::BatterySample;
use statline_win::metrics::battery;

pub fn sample() -> Option<BatterySample> {
    let reading = battery::battery()?;
    Some(BatterySample {
        percent: reading.percent,
        on_ac: reading.on_ac,
        seconds_left: reading.seconds_left,
        rate_mw: reading.rate_mw,
        designed_mwh: None,
        full_mwh: None,
        temperature_c: None,
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn sample_is_optional_but_valid_when_present() {
        if let Some(battery) = super::sample() {
            assert!((0.0..=100.0).contains(&battery.percent));
        }
    }
}
