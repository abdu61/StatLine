//! Metric providers and the sampling engine.
//!
//! Populated in M1. Kept as a separate crate so the sampler can be benchmarked
//! and tested without any UI or window code.

/// Placeholder until the first provider lands in M1.
pub fn crate_ready() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #[test]
    fn crate_builds() {
        assert!(super::crate_ready());
    }
}
