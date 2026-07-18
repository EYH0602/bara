//! Dependencies panel — the first consumer of the shared [`Modal`].
//!
//! A header launcher button shows a live count of the manifest's
//! `related_work`; clicking it opens a [`Modal`] listing every reference in
//! source order. The panel carries its own case-insensitive filter input. A 0
//! count hides the launcher entirely.
//!
//! Like the rest of the viewer this compiles on native and wasm; the pure
//! [`rw_matches`] filter predicate is native-testable.

use ara_core::RelatedWork;
use leptos::prelude::*;

use crate::modal::Modal;
use crate::state::LoadState;

/// Case-insensitive substring match of `query` against a related-work entry.
///
/// The haystack is the entry's id, cite, kind, doi, delta (`what_changed` /
/// `why`), adopted elements, and affected claim ids. An empty/whitespace query
/// matches everything.
pub fn rw_matches(rw: &RelatedWork, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    let mut hay = String::new();
    hay.push_str(&rw.id);
    hay.push(' ');
    hay.push_str(&rw.cite);
    hay.push(' ');
    for s in [&rw.kind, &rw.doi, &rw.what_changed, &rw.why, &rw.adopted]
        .into_iter()
        .flatten()
    {
        hay.push_str(s);
        hay.push(' ');
    }
    for c in &rw.claims_affected {
        hay.push_str(c.as_str());
        hay.push(' ');
    }
    hay.to_lowercase().contains(&q)
}

/// The Dependencies launcher + its modal.
///
/// Renders into the header `.panel-launchers` container. The launcher button is
/// omitted when the loaded manifest has no related work; otherwise it shows the
/// live count and toggles the modal.
#[component]
pub fn DependenciesPanel(load_state: ReadSignal<LoadState>) -> impl IntoView {
    let open = RwSignal::new(false);
    let query = RwSignal::new(String::new());

    // The manifest's related work, refreshed on every load-state change.
    let related_work = Memo::new(move |_| match load_state.get() {
        LoadState::Loaded(m) => m.related_work.clone(),
        _ => Vec::new(),
    });

    view! {
        // Launcher: hidden entirely at a 0 count.
        {move || {
            let count = related_work.get().len();
            (count > 0).then(|| view! {
                <button
                    type="button"
                    class="btn panel-launch-btn"
                    on:click=move |_| open.update(|o| *o = !*o)
                >
                    "Dependencies"
                    <span class="launch-count">{count}</span>
                </button>
            })
        }}

        <Modal open=open title="Dependencies">
            <input
                class="panel-filter"
                type="text"
                placeholder="filter\u{2026}"
                aria-label="Filter dependencies"
                prop:value=move || query.get()
                on:input=move |ev| query.set(event_target_value(&ev))
            />
            // Reactive list: re-filtered on every keystroke, source order kept.
            {move || {
                let q = query.get();
                let items: Vec<RelatedWork> = related_work
                    .get()
                    .into_iter()
                    .filter(|rw| rw_matches(rw, &q))
                    .collect();
                if items.is_empty() {
                    view! {
                        <p class="rw-empty">"No dependencies match the filter."</p>
                    }
                    .into_any()
                } else {
                    view! {
                        <div class="rw-list">
                            {items.into_iter().map(rw_entry).collect::<Vec<_>>()}
                        </div>
                    }
                    .into_any()
                }
            }}
        </Modal>
    }
}

/// Render one related-work entry as a `.block` card.
fn rw_entry(rw: RelatedWork) -> impl IntoView {
    // Delta: shown when either half is present.
    let has_delta = rw.what_changed.is_some() || rw.why.is_some();
    view! {
        <div class="block rw-entry">
            <div class="rw-head">
                <span class="rw-id">{rw.id.clone()}</span>
                <span class="rw-cite">{rw.cite.clone()}</span>
            </div>
            {rw.kind.clone().map(|k| view! {
                <div class="rw-line">
                    <span class="rw-key">"Type"</span>
                    <span>{k}</span>
                </div>
            })}
            {rw.doi.clone().map(|d| view! {
                <div class="rw-line">
                    <span class="rw-key">"DOI"</span>
                    <span>{d}</span>
                </div>
            })}
            {has_delta.then(|| view! {
                <div class="rw-line">
                    <span class="rw-key">"Delta"</span>
                    <span>
                        {rw.what_changed.clone().unwrap_or_default()}
                        {rw.why.clone().map(|w| view! {
                            <span class="rw-why">" \u{2014} "{w}</span>
                        })}
                    </span>
                </div>
            })}
            {rw.adopted.clone().map(|a| view! {
                <div class="rw-line">
                    <span class="rw-key">"Adopted"</span>
                    <span>{a}</span>
                </div>
            })}
            {(!rw.claims_affected.is_empty()).then(|| view! {
                <div class="rw-line">
                    <span class="rw-key">"Claims"</span>
                    <div class="chip-row">
                        {rw.claims_affected.iter().map(|c| view! {
                            <span class="chip">{c.as_str().to_string()}</span>
                        }).collect::<Vec<_>>()}
                    </div>
                </div>
            })}
        </div>
    }
}

// ── Unit tests (native — no browser required) ─────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use ara_core::ClaimId;

    fn rw(id: &str, cite: &str) -> RelatedWork {
        RelatedWork {
            id: id.to_string(),
            cite: cite.to_string(),
            doi: None,
            kind: None,
            what_changed: None,
            why: None,
            adopted: None,
            claims_affected: vec![],
        }
    }

    #[test]
    fn empty_query_matches_everything() {
        let r = rw("RW01", "Vaswani et al., 2017");
        assert!(rw_matches(&r, ""));
        assert!(rw_matches(&r, "   "));
    }

    #[test]
    fn matches_id_and_cite_case_insensitively() {
        let r = rw("RW01", "Vaswani et al., 2017");
        assert!(rw_matches(&r, "rw01"));
        assert!(rw_matches(&r, "VASWANI"));
        assert!(!rw_matches(&r, "resnet"));
    }

    #[test]
    fn matches_secondary_fields() {
        let r = RelatedWork {
            kind: Some("baseline, extends".to_string()),
            doi: Some("10.1000/xyz".to_string()),
            what_changed: Some("adds relative encoding".to_string()),
            why: Some("better on long sequences".to_string()),
            adopted: Some("multi-head attention".to_string()),
            claims_affected: vec![ClaimId::new("C07")],
            ..rw("RW02", "He et al., 2016")
        };
        assert!(rw_matches(&r, "baseline"));
        assert!(rw_matches(&r, "10.1000"));
        assert!(rw_matches(&r, "relative"));
        assert!(rw_matches(&r, "long sequences"));
        assert!(rw_matches(&r, "multi-head"));
        assert!(rw_matches(&r, "c07"));
    }
}
