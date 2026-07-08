//! `ara-core`: the shared core of the ARA viewer runtime.
//!
//! This crate holds all parsing, normalization, binding resolution, and DAG
//! layout for the ARA viewer. It is compiled to both native targets (used by
//! `ara-cli`) and `wasm32-unknown-unknown` (used by the browser client), so it
//! is the single source of truth that keeps the server and client from
//! drifting.
//!
//! This is a skeleton reservation release; real functionality lands in a later
//! version. See <https://github.com/EYH0602/bara>.

/// Returns the version of `ara-core`, taken from the crate manifest.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_reported() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
