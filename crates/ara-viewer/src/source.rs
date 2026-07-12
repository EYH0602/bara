//! Manifest source seam — where the viewer gets its `Manifest` from.
//!
//! Stage 4 adds the [`ManifestSource::Api`] variant: the viewer first tries the
//! `ara serve` JSON endpoint and, on network error / 404, falls back to a static
//! `manifest.json`. The same wasm bundle therefore works under both `ara serve`
//! (live) and any static host (`trunk serve`, GitHub Pages) with no rebuild.
//!
//! Live reload is a companion WebSocket ([`connect_live`]): under `ara serve`
//! the server pushes on every reparse and the client re-fetches; under a static
//! host the socket never opens and live reload is simply inert.

use crate::state::LoadState;
#[cfg(target_arch = "wasm32")]
use crate::state::parse_manifest;

/// Where the viewer fetches its [`ara_core::Manifest`].
// Field reads happen only on the wasm fetch/live paths.  On native (cargo test)
// those branches are cfg'd out, so rustc reports the fields as dead; the allow
// suppresses the spurious warning.
#[allow(dead_code)]
#[derive(Clone)]
pub enum ManifestSource {
    /// Fetch a checked-in JSON file by URL — no live reload.
    Static(String),
    /// Live mode: try `manifest_url`, fall back to `fallback_url` on failure,
    /// and subscribe to `live_url` for reparse notifications.
    Api {
        /// Primary endpoint served by `ara serve` (`/api/manifest`).
        manifest_url: String,
        /// Static manifest used when the primary endpoint is absent.
        fallback_url: String,
        /// WebSocket endpoint that emits on every server-side reparse.
        live_url: String,
    },
}

impl Default for ManifestSource {
    /// The default source is live-with-fallback: `ara serve` under
    /// `api/manifest` + `api/live`, falling back to the static
    /// `manifest.json` that Trunk copies into `dist/`.
    ///
    /// All three URLs are **relative** so they resolve against the document
    /// base (`document.baseURI`). Under local `ara serve` the page is at `/`,
    /// so `api/manifest` → `/api/manifest` (unchanged behaviour). Under the hub
    /// the served `index.html` carries `<base href="/a/{id}/">`, so the same
    /// relative URL → `/a/{id}/api/manifest`. See `plans`/`docs` D1.
    fn default() -> Self {
        Self::Api {
            manifest_url: "api/manifest".into(),
            fallback_url: "manifest.json".into(),
            live_url: "api/live".into(),
        }
    }
}

/// Fetch the manifest described by `source` and write the result into `set_state`.
///
/// For [`ManifestSource::Api`] this tries `manifest_url` first and falls back to
/// `fallback_url` on any network error or non-2xx response, so the same bundle
/// serves both `ara serve` and static hosting.
///
/// Compiled **only for `wasm32-unknown-unknown`**; on native it is a no-op stub
/// so `cargo test` compiles without browser deps.
#[cfg(target_arch = "wasm32")]
pub fn fetch_manifest(source: ManifestSource, set_state: impl Fn(LoadState) + 'static) {
    use wasm_bindgen_futures::spawn_local;

    let (primary, fallback) = match source {
        ManifestSource::Static(url) => (url, None),
        ManifestSource::Api {
            manifest_url,
            fallback_url,
            ..
        } => (manifest_url, Some(fallback_url)),
    };

    spawn_local(async move {
        // Try the primary URL; on transport failure or non-2xx, try the
        // fallback (when one is configured) before surfacing an error.
        let text = match fetch_text(&primary).await {
            Ok(t) => Ok(t),
            Err(primary_err) => match &fallback {
                Some(url) => fetch_text(url).await.map_err(|_| primary_err),
                None => Err(primary_err),
            },
        };

        match text {
            Ok(body) => match parse_manifest(&body) {
                Ok(manifest) => set_state(LoadState::Loaded(manifest)),
                Err(reason) => set_state(LoadState::Failed(format!("Parse error: {reason}"))),
            },
            Err(reason) => set_state(LoadState::Failed(reason)),
        }
    });
}

/// GET `url` and return the body text, or a human-readable error string.
#[cfg(target_arch = "wasm32")]
async fn fetch_text(url: &str) -> Result<String, String> {
    let response = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    if !response.ok() {
        return Err(format!("{} {}", response.status(), response.status_text()));
    }

    response
        .text()
        .await
        .map_err(|e| format!("Failed to read response body: {e}"))
}

/// Subscribe to server-side reparse notifications and re-fetch on each message.
///
/// Opens the `/api/live` WebSocket (only for [`ManifestSource::Api`]) and, on
/// every message, re-runs [`fetch_manifest`] with the same `source` so the graph
/// updates in place. Pan/zoom/selection survive because those signals live in
/// `App` and are untouched by the manifest swap.
///
/// If the socket cannot open (static host), the task ends quietly — live reload
/// is inert, not an error.
#[cfg(target_arch = "wasm32")]
pub fn connect_live(source: ManifestSource, set_state: impl Fn(LoadState) + Clone + 'static) {
    use futures_util::StreamExt;
    use wasm_bindgen_futures::spawn_local;

    let ManifestSource::Api { live_url, .. } = &source else {
        return;
    };

    let ws_url = match absolute_ws_url(live_url) {
        Some(u) => u,
        None => return,
    };

    let ws = match gloo_net::websocket::futures::WebSocket::open(&ws_url) {
        Ok(ws) => ws,
        Err(_) => return, // No live server (static host) — inert.
    };

    spawn_local(async move {
        let mut ws = ws;
        while let Some(msg) = ws.next().await {
            // Any message (the server sends the new ETag) means "reparsed" —
            // re-fetch and re-render. Errors end the stream; we just stop.
            if msg.is_err() {
                break;
            }
            fetch_manifest(source.clone(), set_state.clone());
        }
    });
}

/// Resolve a relative path (e.g. `api/live`) to an absolute `ws://` / `wss://`
/// URL against the document base. Returns `None` if the document base is
/// unavailable (non-browser context).
///
/// Reads `document.baseURI`, which reflects any `<base>` tag: under local
/// `ara serve` the base is the origin root, so `api/live` → `ws://host/api/live`;
/// under the hub the served page carries `<base href="/a/{id}/">`, so the same
/// relative path → `ws://host/a/{id}/api/live`. The pure resolution + scheme
/// swap lives in [`ws_url_from_base`] so it is testable with a synthetic base.
#[cfg(target_arch = "wasm32")]
fn absolute_ws_url(path: &str) -> Option<String> {
    let base = web_sys::window()?.document()?.base_uri().ok()??;
    ws_url_from_base(&base, path)
}

/// Resolve `path` against `base` and convert the resulting `http(s)` URL to a
/// `ws://` / `wss://` URL. Split out of [`absolute_ws_url`] so the load-bearing
/// D1 resolution (relative path + `<base>`) is unit-testable with a synthetic
/// base rather than the live `document.baseURI`.
///
/// `https:` maps to `wss`, anything else (`http:`, `file:`) to `ws`. The `host`
/// component already carries the port when present (e.g. `localhost:8080`).
#[cfg(target_arch = "wasm32")]
pub fn ws_url_from_base(base: &str, path: &str) -> Option<String> {
    let resolved = web_sys::Url::new_with_base(path, base).ok()?;
    let scheme = match resolved.protocol().as_str() {
        "https:" => "wss",
        _ => "ws",
    };
    Some(format!(
        "{scheme}://{}{}{}",
        resolved.host(),
        resolved.pathname(),
        resolved.search()
    ))
}

/// Native stub — the viewer never runs natively; the stub keeps `cargo test`
/// compiling without pulling in any wasm-only dependencies.
#[cfg(not(target_arch = "wasm32"))]
pub fn fetch_manifest(_source: ManifestSource, _set_state: impl Fn(LoadState) + 'static) {
    // No-op on native: network fetch is browser-only.
}

/// Native stub — see [`fetch_manifest`].
#[cfg(not(target_arch = "wasm32"))]
pub fn connect_live(_source: ManifestSource, _set_state: impl Fn(LoadState) + Clone + 'static) {
    // No-op on native: WebSocket live reload is browser-only.
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wire-contract guard: the default source must use **relative** URLs so
    /// they resolve against `document.baseURI` (D1). An accidental leading `/`
    /// would re-hardcode absolute paths and break hub sub-path serving; the
    /// browser test in `tests/web.rs` proves the resolution end-to-end.
    #[test]
    fn default_source_urls_are_relative() {
        match ManifestSource::default() {
            ManifestSource::Api {
                manifest_url,
                fallback_url,
                live_url,
            } => {
                assert_eq!(manifest_url, "api/manifest");
                assert_eq!(fallback_url, "manifest.json");
                assert_eq!(live_url, "api/live");
                for url in [&manifest_url, &fallback_url, &live_url] {
                    assert!(
                        !url.starts_with('/'),
                        "default URL {url:?} must be relative (no leading '/')"
                    );
                }
            }
            ManifestSource::Static(_) => panic!("default must be the Api variant"),
        }
    }
}
