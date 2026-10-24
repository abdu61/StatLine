//! Direct2D/DirectWrite rendering, design tokens and the immediate-mode
//! widget kit.
//!
//! Populated in M2. All drawing goes through a software D2D render target so
//! the app never holds a GPU device open.

/// Placeholder until the render pipeline lands in M2.
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
