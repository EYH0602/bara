//! ARA Viewer — Leptos CSR entry point.
//!
//! Mounts the [`App`] component to `<body>`. All application logic lives in
//! sub-components; this file is intentionally minimal.

mod kind;

use leptos::prelude::*;

fn main() {
    console_error_panic_hook::set_once();
    mount_to_body(App);
}

/// Root application shell.
///
/// Renders the two-pane layout: a fixed header with title and toolbar area,
/// and a CSS grid main section containing the `#map` (left) and `#detail`
/// (right) panels.
#[component]
fn App() -> impl IntoView {
    view! {
        <header class="app-header">
            <div class="header-title">
                <h1>"ARA Viewer"</h1>
                <span class="header-subtitle">"Agent-Native Research Artifact"</span>
            </div>
            <div class="toolbar-area">
                // Toolbar placeholder — populated in a later step.
            </div>
        </header>
        <main class="app-main">
            <section id="map" class="panel panel-map">
                <p class="placeholder-text">"Graph goes here"</p>
            </section>
            <section id="detail" class="panel panel-detail">
                <p class="placeholder-text">"Select a step on the left."</p>
            </section>
        </main>
    }
}
