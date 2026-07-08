//! `ara`: command-line entry point for the ARA viewer runtime.
//!
//! Installed via `cargo install ara-cli`, this ships a binary named `ara`.
//! This is a skeleton reservation release; `validate` and `serve` land in a
//! later version. See <https://github.com/EYH0602/bara>.

fn main() {
    println!(
        "ara {} — ARA viewer runtime (skeleton). See https://github.com/EYH0602/bara",
        env!("CARGO_PKG_VERSION")
    );
}
