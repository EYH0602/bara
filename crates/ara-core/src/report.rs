//! Diagnostics produced by parsing/validation.
//!
//! A [`Diagnostic`] carries a **logical** path (e.g. `nodes[N07].evidence[0]`),
//! not a source `line:column` — `serde-saphyr` does not expose reliable spans
//! through serde, so line numbers are intentionally not promised.

use serde::Serialize;

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warning,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Error => f.write_str("error"),
            Severity::Warning => f.write_str("warning"),
        }
    }
}

/// A single diagnostic: severity, logical path, and message.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}: {}", self.severity, self.path, self.message)
    }
}

/// The outcome of a parse: separated errors and warnings.
///
/// A parse "succeeds" (`is_ok`) when there are no errors — warnings do not
/// block success but **must** still be surfaced by callers.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ParseReport {
    errors: Vec<Diagnostic>,
    warnings: Vec<Diagnostic>,
}

impl ParseReport {
    /// Records an error.
    pub(crate) fn error(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.errors.push(Diagnostic {
            severity: Severity::Error,
            path: path.into(),
            message: message.into(),
        });
    }

    /// Records a warning.
    pub(crate) fn warn(&mut self, path: impl Into<String>, message: impl Into<String>) {
        self.warnings.push(Diagnostic {
            severity: Severity::Warning,
            path: path.into(),
            message: message.into(),
        });
    }

    /// All errors, in the order they were recorded.
    pub fn errors(&self) -> &[Diagnostic] {
        &self.errors
    }

    /// All warnings, in the order they were recorded.
    pub fn warnings(&self) -> &[Diagnostic] {
        &self.warnings
    }

    /// True when there are no errors (warnings are allowed).
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

impl std::fmt::Display for ParseReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for d in &self.errors {
            writeln!(f, "{d}")?;
        }
        for d in &self.warnings {
            writeln!(f, "{d}")?;
        }
        write!(
            f,
            "{} error(s), {} warning(s)",
            self.errors.len(),
            self.warnings.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok_and_accessors() {
        let mut r = ParseReport::default();
        assert!(r.is_ok());
        r.warn("document", "empty");
        assert!(r.is_ok()); // warnings don't block
        assert_eq!(r.warnings().len(), 1);
        r.error("nodes[N01]", "duplicate node id");
        assert!(!r.is_ok());
        assert_eq!(r.errors().len(), 1);
    }

    #[test]
    fn diagnostic_display() {
        let d = Diagnostic {
            severity: Severity::Error,
            path: "nodes[N07].evidence[0]".into(),
            message: "unknown claim".into(),
        };
        assert_eq!(
            d.to_string(),
            "error: nodes[N07].evidence[0]: unknown claim"
        );
    }
}
