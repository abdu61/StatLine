//! Views (taskbar strip, flyout, dashboard) and pure layout maths.
//!
//! Populated in M2/M3. Layout functions return plain geometry so they can be
//! unit-tested without a window or a render target.

/// Placeholder until the first view lands.
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
