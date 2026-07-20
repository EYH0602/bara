//! Client-side markdown → HTML for exhibit bodies (issue #32).
//!
//! Exhibit bodies (`evidence/figures/*.md`, `evidence/tables/*.md`) are full GFM
//! markdown — captions + prose + tables. We render them to an HTML string with
//! `pulldown-cmark` and mount it via Leptos's `inner_html` on a wrapper element
//! (approach A). This is the first `inner_html` in the viewer, which otherwise
//! emits only escaped Leptos nodes; the exception is deliberate and bounded to
//! trusted-local exhibit content.
//!
//! Only the extensions the corpus needs are enabled (tables, strikethrough). In
//! particular the math extension is **off**, so `$…$` stays literal — the same
//! inert posture as `latex_view` (D3). Raw HTML embedded in a body is escaped,
//! not passed through, so `inner_html` cannot inject markup.

use pulldown_cmark::{Event, Options, Parser, html};

/// Render an exhibit body's GFM markdown to an HTML string.
///
/// Tables and strikethrough are enabled; raw HTML in the source is neutralised
/// (emitted as escaped text) so the result is safe to mount via `inner_html`.
pub fn render_exhibit_body(md: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);

    // Neutralise raw-HTML events: re-emit their content as escaped text so no
    // author-supplied markup reaches the DOM through `inner_html`.
    let parser = Parser::new_ext(md, opts).map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => Event::Text(raw),
        other => other,
    });

    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_gfm_table() {
        let md = "| a | b |\n|---|---|\n| 1 | 2 |\n";
        let html = render_exhibit_body(md);
        assert!(html.contains("<table>"), "got: {html}");
        assert!(html.contains("<th>"), "got: {html}");
        assert!(html.contains("<td>"), "got: {html}");
    }

    #[test]
    fn escapes_raw_html() {
        let html = render_exhibit_body("<script>alert(1)</script>");
        assert!(!html.contains("<script>"), "raw HTML leaked: {html}");
        assert!(html.contains("&lt;script&gt;"), "not escaped: {html}");
    }

    #[test]
    fn leaves_math_literal() {
        let html = render_exhibit_body("energy is $E = mc^2$ here");
        assert!(html.contains("$E = mc^2$"), "math not literal: {html}");
    }
}
