//! ARA Viewer — binary entry point.
//!
//! Thin wrapper that delegates to [`ara_viewer::mount`].  All application
//! logic lives in the library target (`src/lib.rs` and its sub-modules) so
//! the `wasm-bindgen-test` browser-test layer can import components directly.

fn main() {
    ara_viewer::mount();
}
