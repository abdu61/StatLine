//! Network rate math.
//!
//! Counters are cumulative byte totals per interface. Rates come from deltas
//! against the previous tick, divided by the measured wall-clock interval —
//! never by the configured interval, because timer coalescing and sleep/resume
//! both change it.

use statline_core::model::{AdapterSample, NetworkSample};
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Default)]
pub struct NetworkState {
    previous: HashMap<String, (u64, u64)>,
    session_rx: u64,
    session_tx: u64,
}

impl NetworkState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold a fresh set of interface counters into a `NetworkSample`.
    ///
    /// `elapsed` of `None` means "no interval yet": rates are zero but session
    /// totals still update.
    pub fn update(
        &mut self,
        interfaces: Vec<(String, u64, u64)>,
        elapsed: Option<Duration>,
    ) -> NetworkSample {
        let seconds = elapsed.map(|d| d.as_secs_f64()).filter(|s| *s > 0.0);

        let mut adapters = Vec::with_capacity(interfaces.len());
        let mut total_rx = 0.0;
        let mut total_tx = 0.0;

        for (name, in_octets, out_octets) in interfaces {
            let (rx, tx) = match (self.previous.get(&name), seconds) {
                (Some(&(prev_in, prev_out)), Some(seconds)) => {
                    let rx = in_octets.saturating_sub(prev_in) as f64 / seconds;
                    let tx = out_octets.saturating_sub(prev_out) as f64 / seconds;
                    self.session_rx = self
                        .session_rx
                        .saturating_add(in_octets.saturating_sub(prev_in));
                    self.session_tx = self
                        .session_tx
                        .saturating_add(out_octets.saturating_sub(prev_out));
                    (rx, tx)
                }
                _ => (0.0, 0.0),
            };

            self.previous.insert(name.clone(), (in_octets, out_octets));
            total_rx += rx;
            total_tx += tx;
            adapters.push(AdapterSample {
                name,
                rx_bytes_per_sec: rx,
                tx_bytes_per_sec: tx,
            });
        }

        // Forget interfaces that disappeared so a re-plug starts clean.
        let live: Vec<String> = adapters.iter().map(|a| a.name.clone()).collect();
        self.previous.retain(|name, _| live.contains(name));

        adapters.sort_by(|a, b| a.name.cmp(&b.name));

        NetworkSample {
            rx_bytes_per_sec: total_rx,
            tx_bytes_per_sec: total_tx,
            adapters,
            session_rx_bytes: self.session_rx,
            session_tx_bytes: self.session_tx,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn iface(name: &str, rx: u64, tx: u64) -> (String, u64, u64) {
        (name.to_string(), rx, tx)
    }

    #[test]
    fn first_sample_reports_zero_rates() {
        let mut state = NetworkState::new();
        let sample = state.update(vec![iface("Wi-Fi", 1_000, 2_000)], None);
        assert_eq!(sample.rx_bytes_per_sec, 0.0);
        assert_eq!(sample.tx_bytes_per_sec, 0.0);
    }

    #[test]
    fn second_sample_divides_by_elapsed_time() {
        let mut state = NetworkState::new();
        let _ = state.update(vec![iface("Wi-Fi", 0, 0)], None);
        let sample = state.update(
            vec![iface("Wi-Fi", 2_000, 1_000)],
            Some(Duration::from_secs(2)),
        );
        assert_eq!(sample.rx_bytes_per_sec, 1_000.0);
        assert_eq!(sample.tx_bytes_per_sec, 500.0);
        assert_eq!(sample.session_rx_bytes, 2_000);
        assert_eq!(sample.session_tx_bytes, 1_000);
    }

    #[test]
    fn new_interfaces_do_not_produce_spurious_spikes() {
        let mut state = NetworkState::new();
        let _ = state.update(vec![iface("Ethernet", 5, 5)], None);
        let sample = state.update(
            vec![
                iface("Ethernet", 10, 10),
                iface("Wi-Fi", 9_999_999, 9_999_999),
            ],
            Some(Duration::from_secs(1)),
        );
        assert_eq!(sample.rx_bytes_per_sec, 5.0);
        let wifi = sample
            .adapters
            .iter()
            .find(|a| a.name == "Wi-Fi")
            .expect("wifi adapter");
        assert_eq!(wifi.rx_bytes_per_sec, 0.0);
    }

    #[test]
    fn counter_reset_does_not_underflow() {
        let mut state = NetworkState::new();
        let _ = state.update(vec![iface("Wi-Fi", 10_000, 10_000)], None);
        let sample = state.update(vec![iface("Wi-Fi", 100, 100)], Some(Duration::from_secs(1)));
        assert_eq!(sample.rx_bytes_per_sec, 0.0);
        assert_eq!(sample.session_rx_bytes, 0);
    }

    #[test]
    fn zero_elapsed_is_treated_as_no_interval() {
        let mut state = NetworkState::new();
        let _ = state.update(vec![iface("Wi-Fi", 0, 0)], None);
        let sample = state.update(
            vec![iface("Wi-Fi", 1_000, 1_000)],
            Some(Duration::from_secs(0)),
        );
        assert_eq!(sample.rx_bytes_per_sec, 0.0);
    }
}
