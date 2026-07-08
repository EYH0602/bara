//! `ara-viewer`: memorable front-door for the ARA viewer runtime.
//!
//! Reserved umbrella name. The command-line tool ships as the `ara` binary from
//! the `ara-cli` crate; install it with `cargo install ara-cli`. This is a
//! skeleton reservation release. See <https://github.com/EYH0602/bara>.

fn main() {
    println!(
        "ara-viewer {} — install the CLI with `cargo install ara-cli`, then run `ara`.",
        env!("CARGO_PKG_VERSION")
    );
}
