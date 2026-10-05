//! The diagnostic DTO: a content problem with a stable code and, when it concerns one input field, the JSON
//! pointer of that field.
//!
//! Conversion from `rimstudio_core::diag::Diagnostic` lives here because it needs nothing beyond core. The
//! designer validation puts the pointer of the field in the `field` argument; the DTO lifts it into its own
//! `field` member so the webview can attach the message to the input without reading `args`.

use std::collections::BTreeMap;

use rimstudio_core::diag::{Diagnostic, DiagnosticSummary, Severity, Span};
use serde::{Deserialize, Serialize};

/// The name of the diagnostic argument that carries a field pointer.
pub const FIELD_ARG: &str = "field";

/// How serious a diagnostic is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum SeverityDto {
    /// Blocks the operation.
    Error,
    /// Should be looked at.
    Warning,
    /// Informational.
    Info,
    /// A suggestion.
    Hint,
}

impl From<Severity> for SeverityDto {
    fn from(value: Severity) -> Self {
        match value {
            Severity::Error => Self::Error,
            Severity::Warning => Self::Warning,
            Severity::Info => Self::Info,
            Severity::Hint => Self::Hint,
        }
    }
}

/// Where inside a file a diagnostic points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum SpanDto {
    /// A one based line and column.
    LineCol {
        /// One based line number.
        line: u32,
        /// One based column number.
        column: u32,
    },
    /// A byte range `[start, end)`.
    Bytes {
        /// First byte.
        start: u32,
        /// One past the last byte.
        end: u32,
    },
}

impl From<Span> for SpanDto {
    fn from(value: Span) -> Self {
        match value {
            Span::LineCol { line, column } => Self::LineCol { line, column },
            Span::Bytes { start, end } => Self::Bytes { start, end },
        }
    }
}

/// One content problem.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticDto {
    /// Stable code, `<area>.<kebab-name>`; the UI translates by code.
    pub code: String,
    /// How serious it is.
    pub severity: SeverityDto,
    /// English fallback text.
    pub message: String,
    /// JSON pointer of the input field the problem concerns, for example `/ranged/damage`. Absent when it
    /// concerns the whole input or a file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub field: Option<String>,
    /// Session handle of the mod it concerns. Absent when it concerns no mod.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mod_idx: Option<u32>,
    /// Session handle of the file it concerns. Absent when it concerns no file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub file_id: Option<u32>,
    /// Position inside the file. Absent when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub span: Option<SpanDto>,
    /// Values for the translated message template, ordered by key; never contains the `field` argument.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<BTreeMap<String, String>>", optional))]
    pub args: BTreeMap<String, String>,
}

impl From<&Diagnostic> for DiagnosticDto {
    fn from(value: &Diagnostic) -> Self {
        let mut args = value.args.clone();
        let field = args.remove(FIELD_ARG);
        Self {
            code: value.code.as_str().to_owned(),
            severity: value.severity.into(),
            message: value.message.clone(),
            field,
            mod_idx: value
                .mod_idx
                .and_then(|idx| u32::try_from(idx.index()).ok()),
            file_id: value.file.map(|f| f.get()),
            span: value.span.map(SpanDto::from),
            args,
        }
    }
}

impl From<Diagnostic> for DiagnosticDto {
    fn from(value: Diagnostic) -> Self {
        Self::from(&value)
    }
}

/// Converts a slice of core diagnostics, keeping order.
#[must_use]
pub fn diagnostics_to_dtos(items: &[Diagnostic]) -> Vec<DiagnosticDto> {
    items.iter().map(DiagnosticDto::from).collect()
}

/// Counts per code plus the first samples of each code, as a sink produced them.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSummaryDto {
    /// How many diagnostics each code produced, ordered by code.
    pub counts: BTreeMap<String, u64>,
    /// The kept samples.
    pub samples: Vec<DiagnosticDto>,
    /// True when some diagnostics were counted but not kept as samples.
    pub truncated: bool,
    /// Number of error severity diagnostics.
    pub errors: u64,
    /// Number of warning severity diagnostics.
    pub warnings: u64,
    /// Number of info severity diagnostics.
    pub infos: u64,
    /// Number of hint severity diagnostics.
    pub hints: u64,
}

impl From<&DiagnosticSummary> for DiagnosticSummaryDto {
    fn from(value: &DiagnosticSummary) -> Self {
        Self {
            counts: value
                .counts
                .iter()
                .map(|(code, n)| (code.as_str().to_owned(), *n))
                .collect(),
            samples: diagnostics_to_dtos(&value.samples),
            truncated: value.truncated,
            errors: value.errors,
            warnings: value.warnings,
            infos: value.infos,
            hints: value.hints,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::diag::DiagCode;

    fn sample() -> Diagnostic {
        Diagnostic {
            code: DiagCode::new("design.out-of-range"),
            severity: Severity::Warning,
            mod_idx: None,
            file: None,
            message: "Damage is out of range.".into(),
            span: Some(Span::LineCol { line: 3, column: 9 }),
            args: BTreeMap::from([
                ("field".to_owned(), "/ranged/damage".to_owned()),
                ("max".to_owned(), "500".to_owned()),
            ]),
        }
    }

    #[test]
    fn field_argument_is_lifted_out_of_args() {
        let dto = DiagnosticDto::from(&sample());
        assert_eq!(dto.field.as_deref(), Some("/ranged/damage"));
        assert!(!dto.args.contains_key(FIELD_ARG));
        assert_eq!(dto.args.get("max").map(String::as_str), Some("500"));
        assert_eq!(dto.severity, SeverityDto::Warning);
        assert_eq!(dto.span, Some(SpanDto::LineCol { line: 3, column: 9 }));
    }

    #[test]
    fn dto_round_trips_through_json() {
        let dto = DiagnosticDto::from(&sample());
        let text = serde_json::to_string(&dto).unwrap_or_default();
        let back: Result<DiagnosticDto, _> = serde_json::from_str(&text);
        assert_eq!(back.ok(), Some(dto));
    }

    #[test]
    fn absent_members_are_omitted() {
        let mut d = sample();
        d.args.clear();
        d.span = None;
        let text = serde_json::to_string(&DiagnosticDto::from(&d)).unwrap_or_default();
        assert!(!text.contains("modIdx"));
        assert!(!text.contains("span"));
        assert!(!text.contains("args"));
    }
}
