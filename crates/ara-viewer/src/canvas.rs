//! `CanvasRenderer` stub — reserved for a potential future swap.
//!
//! If the Step-8 fps probe shows that the SVG DOM approach cannot sustain
//! 60 fps on the largest ARA corpus (~34 nodes / ~50 edges), this module will
//! be promoted to a full renderer that draws the same [`GraphScene`] onto a
//! `web-sys` `CanvasRenderingContext2d` obtained from a Leptos `NodeRef<Canvas>`.
//!
//! Until then this file compiles to essentially nothing on both native and wasm.

use crate::scene::GraphScene;

/// Canvas-based renderer stub.
///
/// Will use `web-sys` `CanvasRenderingContext2d` (from a `NodeRef<Canvas>`)
/// when promoted.  Not wired into the UI at this stage.
pub struct CanvasRenderer;

impl CanvasRenderer {
    /// Stub render method.  Does nothing until the Step-8 fps gate is crossed.
    #[allow(dead_code)]
    pub fn render(&self, _scene: &GraphScene) {
        // Not implemented — canvas rendering is deferred pending fps probe.
    }
}
