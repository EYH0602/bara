//! Shared, accessible `Modal` dialog — the reusable overlay behind every
//! side-panel (Dependencies, and the Glossary / Context / Recipes panels that
//! land next).
//!
//! Like the rest of the viewer the component compiles on **both** native and
//! wasm32.  The DOM structure (scrim + dialog card + header + body) renders on
//! either target so native component tests compile; the browser-only bits — the
//! Esc/Tab keydown listener, the focus trap, and focus-restore — are gated with
//! `#[cfg(target_arch = "wasm32")]`, mirroring `replay.rs`.
//!
//! ## Accessibility contract (headline feature — see tests/web.rs)
//! - `role="dialog"` + `aria-modal="true"` + `aria-labelledby` → the title.
//! - On open, focus moves into the dialog (the `tabindex="-1"` card).
//! - **Focus trap:** Tab / Shift+Tab wrap at both ends; focus cannot leave.
//! - **Esc** closes; the **scrim click** closes; clicking the card does not.
//! - **Focus restore:** on close, focus returns to the element that opened it
//!   (captured into a `StoredValue`, so it survives Leptos re-renders).

use core::sync::atomic::{AtomicUsize, Ordering};

use leptos::prelude::*;

/// Process-wide sequence so every mounted `Modal` gets unique element ids
/// (`ara-modal-{n}` / `ara-modal-title-{n}`).  The wasm focus/keydown code
/// looks the dialog up by id, so the ids must not collide when two modals mount.
static MODAL_SEQ: AtomicUsize = AtomicUsize::new(0);

/// Thread-local store for the live document `keydown` closure (non-`Send`, so
/// `LocalStorage`). `Some` only while the modal is open.
#[cfg(target_arch = "wasm32")]
type KeydownListener = StoredValue<
    Option<leptos::wasm_bindgen::prelude::Closure<dyn FnMut(leptos::web_sys::KeyboardEvent)>>,
    leptos::prelude::LocalStorage,
>;

/// A reusable, accessible modal dialog.
///
/// `open` drives visibility (and, on wasm, the focus-trap lifecycle); `title`
/// is the accessible name shown in the header and pointed at by
/// `aria-labelledby`; `children` is the panel body.  `children` is a
/// [`ChildrenFn`] so the body re-renders reactively (the Dependencies panel's
/// filter depends on that).
#[component]
pub fn Modal(
    open: RwSignal<bool>,
    #[prop(into)] title: String,
    children: ChildrenFn,
) -> impl IntoView {
    let seq = MODAL_SEQ.fetch_add(1, Ordering::Relaxed);
    let dialog_id = format!("ara-modal-{seq}");
    let title_id = format!("ara-modal-title-{seq}");

    // Ref to the dialog card, so the focus effect can act the instant the card
    // mounts (rather than racing the conditional render via `getElementById`).
    let dialog_ref: NodeRef<leptos::html::Div> = NodeRef::new();

    // ── wasm-only: focus trap + Esc/Tab listener + focus restore ──────────────
    // Driven by an Effect watching both `open` and `dialog_ref`. On open the
    // effect first runs with the card unmounted (ref = None) and does nothing;
    // when the card mounts the ref updates and the effect re-runs to capture the
    // pre-open focus, move focus in, and install the keydown listener.
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::wasm_bindgen::JsCast;
        use leptos::wasm_bindgen::prelude::Closure;

        // Live keydown closure (Some only while open) and the element focused
        // before the modal opened.  Both are imperative, non-`Send` browser
        // state, so they live in thread-local `StoredValue`s (not signals).
        let listener: KeydownListener = StoredValue::new_local(None);
        let prev_focus: StoredValue<Option<leptos::web_sys::HtmlElement>, LocalStorage> =
            StoredValue::new_local(None);

        let dialog_id_eff = dialog_id.clone();
        Effect::new(move |_| {
            let is_open = open.get();
            let node = dialog_ref.get();
            // Always drop any previous listener first (idempotent, no leak).
            remove_keydown_listener(listener);
            let Some(doc) = leptos::web_sys::window().and_then(|w| w.document()) else {
                return;
            };

            match (is_open, node) {
                (true, Some(dialog)) => {
                    // Remember what had focus (once) so we can restore it on
                    // close — captured before we move focus into the dialog.
                    if prev_focus.get_value().is_none() {
                        prev_focus.set_value(
                            doc.active_element()
                                .and_then(|e| e.dyn_into::<leptos::web_sys::HtmlElement>().ok()),
                        );
                    }
                    // Move focus into the dialog card (tabindex="-1").
                    let _ = dialog.focus();
                    // Install the Esc-closes + Tab-trap keydown listener.
                    let id_for_key = dialog_id_eff.clone();
                    let handler = Closure::<dyn FnMut(leptos::web_sys::KeyboardEvent)>::new(
                        move |ev: leptos::web_sys::KeyboardEvent| {
                            on_modal_keydown(&ev, open, &id_for_key);
                        },
                    );
                    let _ = doc.add_event_listener_with_callback(
                        "keydown",
                        handler.as_ref().unchecked_ref(),
                    );
                    listener.set_value(Some(handler));
                }
                // Open but not yet mounted: wait for the ref to fire.
                (true, None) => {}
                // Closed: return focus to the pre-open element.
                (false, _) => {
                    if let Some(el) = prev_focus.get_value() {
                        let _ = el.focus();
                    }
                    prev_focus.set_value(None);
                }
            }
        });

        // Tear the listener down on unmount so it can't outlive the component.
        on_cleanup(move || remove_keydown_listener(listener));
    }

    // ── View (rendered on both targets) ───────────────────────────────────────
    // Conditional render: when closed, the whole `.modal-scrim` subtree is
    // removed from the DOM (tests assert `.modal` disappears on Esc). `children`
    // is a ChildrenFn, so it's rebuilt each time the panel opens.
    move || {
        open.get().then({
            let children = children.clone();
            let dialog_id = dialog_id.clone();
            let title_id = title_id.clone();
            let title = title.clone();
            move || {
                let body = children.clone();
                view! {
                    // Scrim: clicking the backdrop itself closes; clicks that
                    // originate inside the card do not (target check below).
                    <div
                        class="modal-scrim"
                        on:click=move |_ev| {
                            #[cfg(target_arch = "wasm32")]
                            if scrim_backdrop_clicked(&_ev) {
                                open.set(false);
                            }
                        }
                    >
                        <div
                            class="modal"
                            node_ref=dialog_ref
                            id=dialog_id.clone()
                            role="dialog"
                            aria-modal="true"
                            aria-labelledby=title_id.clone()
                            tabindex="-1"
                            // Belt-and-suspenders: stop card clicks from bubbling
                            // to the scrim (the target check already guards it).
                            on:click=move |_ev| {
                                #[cfg(target_arch = "wasm32")]
                                _ev.stop_propagation();
                            }
                        >
                            <div class="modal-header">
                                <h2 class="modal-title" id=title_id.clone()>
                                    {title.clone()}
                                </h2>
                                <button
                                    type="button"
                                    class="btn modal-close"
                                    aria-label="Close"
                                    on:click=move |_| open.set(false)
                                >
                                    "\u{2715}"
                                    <span class="modal-esc-hint" aria-hidden="true">"Esc"</span>
                                </button>
                            </div>
                            <div class="modal-body">{body()}</div>
                        </div>
                    </div>
                }
            }
        })
    }
}

// ── wasm-only helpers ─────────────────────────────────────────────────────────

/// Drop the live keydown listener, if any, and remove it from `document`.
/// Idempotent: safe to call when no listener is installed.
#[cfg(target_arch = "wasm32")]
fn remove_keydown_listener(store: KeydownListener) {
    use leptos::wasm_bindgen::JsCast;
    store.update_value(|opt| {
        // `take()` frees the closure regardless; we also detach it from the
        // document when the window is reachable.
        if let Some(cl) = opt.take()
            && let Some(doc) = leptos::web_sys::window().and_then(|w| w.document())
        {
            let _ = doc.remove_event_listener_with_callback("keydown", cl.as_ref().unchecked_ref());
        }
    });
}

/// Handle a document `keydown` while the modal is open: `Escape` closes,
/// `Tab` runs the focus trap. Other keys pass through.
#[cfg(target_arch = "wasm32")]
fn on_modal_keydown(ev: &leptos::web_sys::KeyboardEvent, open: RwSignal<bool>, dialog_id: &str) {
    match ev.key().as_str() {
        "Escape" => {
            ev.prevent_default();
            open.set(false);
        }
        "Tab" => trap_tab(ev, dialog_id),
        _ => {}
    }
}

/// Focus-trap for Tab / Shift+Tab: keep focus cycling among the dialog's
/// focusable descendants, wrapping at both ends. When focus has escaped (active
/// element not inside the dialog) Tab pulls it to the first focusable and
/// Shift+Tab to the last.
#[cfg(target_arch = "wasm32")]
fn trap_tab(ev: &leptos::web_sys::KeyboardEvent, dialog_id: &str) {
    use leptos::wasm_bindgen::JsCast;
    let Some(doc) = leptos::web_sys::window().and_then(|w| w.document()) else {
        return;
    };
    let Some(dialog) = doc.get_element_by_id(dialog_id) else {
        return;
    };
    let focusables = collect_focusable(&dialog);
    if focusables.is_empty() {
        // Nothing to tab to — keep focus pinned on the dialog card.
        ev.prevent_default();
        if let Some(d) = dialog.dyn_ref::<leptos::web_sys::HtmlElement>() {
            let _ = d.focus();
        }
        return;
    }

    let first = &focusables[0];
    let last = focusables.last().unwrap();
    // Index of the currently-focused element within the focusable list.
    let idx = doc.active_element().and_then(|a| {
        focusables
            .iter()
            .position(|f| a.is_same_node(Some(AsRef::<leptos::web_sys::Node>::as_ref(f))))
    });

    let target = if ev.shift_key() {
        // Backward: wrap to last from the first (or from outside the dialog).
        match idx {
            Some(0) | None => Some(last),
            _ => None,
        }
    } else {
        // Forward: wrap to first from the last (or from outside the dialog).
        match idx {
            Some(i) if i + 1 == focusables.len() => Some(first),
            None => Some(first),
            _ => None,
        }
    };

    if let Some(t) = target {
        ev.prevent_default();
        let _ = t.focus();
    }
}

/// Collect the dialog's focusable descendants in DOM order.
#[cfg(target_arch = "wasm32")]
fn collect_focusable(dialog: &leptos::web_sys::Element) -> Vec<leptos::web_sys::HtmlElement> {
    use leptos::wasm_bindgen::JsCast;
    const SEL: &str = "a[href], button:not([disabled]), textarea:not([disabled]), \
         input:not([disabled]), select:not([disabled]), [tabindex]:not([tabindex='-1'])";
    let mut out = Vec::new();
    if let Ok(list) = dialog.query_selector_all(SEL) {
        for i in 0..list.length() {
            if let Some(el) = list
                .item(i)
                .and_then(|n| n.dyn_into::<leptos::web_sys::HtmlElement>().ok())
            {
                out.push(el);
            }
        }
    }
    out
}

/// True when a scrim click landed on the backdrop itself, not on the dialog
/// card or its contents. Robust against Leptos event delegation (which can make
/// `stopPropagation` unreliable) — we key off the click target's class.
#[cfg(target_arch = "wasm32")]
fn scrim_backdrop_clicked(ev: &leptos::web_sys::MouseEvent) -> bool {
    use leptos::wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<leptos::web_sys::Element>().ok())
        .map(|el| el.class_list().contains("modal-scrim"))
        .unwrap_or(false)
}
