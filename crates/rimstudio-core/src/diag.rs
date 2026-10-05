//! Diagnostics: problems in user content, collected instead of raised (invariant I-10).
//!
//! A [`Diagnostic`] names a stable [`DiagCode`], a [`Severity`], optionally the mod and file it
//! concerns, an English fallback message and a small map of arguments the UI uses to render a
//! translated message. A [`DiagSink`] counts every diagnostic per code and keeps a bounded number of
//! samples per code, so a library with 30,000 repeated warnings costs bounded memory and still
//! reports the true totals.
//!
//! Determinism (I-12): samples are not "the first to arrive", which would depend on thread timing.
//! For each code the sink keeps the samples that sort first by (mod, file, span, message, severity,
//! arguments). Merging sinks in any order, from any number of workers, gives identical results.

use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{CoreError, IdFault};
use crate::ids::{FileId, ModIdx};

/// How many samples per code a [`DiagSink`] keeps by default.
pub const DEFAULT_SAMPLE_CAP: usize = 100;

/// How serious a diagnostic is. Ordered from most to least serious.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// Blocks the action or marks the item as broken.
    Error,
    /// A badge and a problems row.
    Warning,
    /// A problems row only.
    Info,
    /// An editor margin hint only.
    Hint,
}

impl Severity {
    /// All severities, most serious first.
    pub const ALL: [Severity; 4] = [
        Severity::Error,
        Severity::Warning,
        Severity::Info,
        Severity::Hint,
    ];

    /// The lowercase name used on the wire.
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
            Severity::Hint => "hint",
        }
    }

    fn slot(self) -> usize {
        match self {
            Severity::Error => 0,
            Severity::Warning => 1,
            Severity::Info => 2,
            Severity::Hint => 3,
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A stable diagnostic code, `<area>.<kebab-name>` in lower case, for example `xml.parse-error`.
///
/// Codes are API: renaming one needs a release note and a locale key update. Producers keep their
/// codes as `const` values built with [`DiagCode::new`].
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DiagCode(Cow<'static, str>);

/// Checks the code grammar: two or more non empty dot separated parts of lower case ASCII letters,
/// digits and single hyphens, each starting with a letter.
fn code_is_well_formed(code: &str) -> bool {
    let mut parts = 0usize;
    for part in code.split('.') {
        parts += 1;
        let bytes = part.as_bytes();
        let Some(first) = bytes.first() else {
            return false;
        };
        if !first.is_ascii_lowercase() {
            return false;
        }
        if bytes.last() == Some(&b'-') {
            return false;
        }
        let mut prev_hyphen = false;
        for &b in bytes {
            let hyphen = b == b'-';
            if !(b.is_ascii_lowercase() || b.is_ascii_digit() || hyphen) || (hyphen && prev_hyphen)
            {
                return false;
            }
            prev_hyphen = hyphen;
        }
    }
    parts >= 2
}

impl DiagCode {
    /// Builds a code from a static string, usable in `const` items.
    ///
    /// The grammar is not checked here (a `const fn` cannot loop cheaply); the `diag` tests of each
    /// producing crate should call [`DiagCode::is_well_formed`] on every code they define.
    pub const fn new(code: &'static str) -> DiagCode {
        DiagCode(Cow::Borrowed(code))
    }

    /// Parses an owned code, checking the grammar.
    ///
    /// # Errors
    /// [`CoreError::InvalidId`] when the text does not follow `<area>.<kebab-name>`.
    pub fn parse(text: &str) -> Result<DiagCode, CoreError> {
        if code_is_well_formed(text) {
            Ok(DiagCode(Cow::Owned(text.to_owned())))
        } else {
            Err(CoreError::invalid_id(
                "diagnostic code",
                text,
                IdFault::BadShape,
            ))
        }
    }

    /// True when the code follows the `<area>.<kebab-name>` grammar.
    pub fn is_well_formed(&self) -> bool {
        code_is_well_formed(&self.0)
    }

    /// The code text.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The part before the first dot, for example `xml`.
    pub fn area(&self) -> &str {
        self.0.split('.').next().unwrap_or("")
    }
}

impl fmt::Display for DiagCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for DiagCode {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for DiagCode {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        DiagCode::parse(&text).map_err(D::Error::custom)
    }
}

/// Where inside a file a diagnostic points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Span {
    /// A one based line and column.
    LineCol {
        /// One based line number.
        line: u32,
        /// One based column number.
        column: u32,
    },
    /// A byte range `[start, end)` into the file.
    Bytes {
        /// First byte.
        start: u32,
        /// One past the last byte.
        end: u32,
    },
}

/// One problem in user content.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    /// The stable code.
    pub code: DiagCode,
    /// How serious it is.
    pub severity: Severity,
    /// The mod it concerns, as a session handle (the IPC layer maps it to a `ModId`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mod_idx: Option<ModIdx>,
    /// The file it concerns, as a session handle (the IPC layer maps it to a relative path).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<FileId>,
    /// An English fallback message; the UI translates by code and arguments.
    pub message: String,
    /// Where in the file it points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    /// Values for the translated message template, in key order.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, String>,
}

impl Diagnostic {
    /// Creates a diagnostic with no mod, file, span or arguments.
    pub fn new(code: DiagCode, severity: Severity, message: impl Into<String>) -> Diagnostic {
        Diagnostic {
            code,
            severity,
            mod_idx: None,
            file: None,
            message: message.into(),
            span: None,
            args: BTreeMap::new(),
        }
    }

    /// Sets the mod this diagnostic concerns.
    #[must_use]
    pub fn with_mod(mut self, mod_idx: ModIdx) -> Diagnostic {
        self.mod_idx = Some(mod_idx);
        self
    }

    /// Sets the file this diagnostic concerns.
    #[must_use]
    pub fn with_file(mut self, file: FileId) -> Diagnostic {
        self.file = Some(file);
        self
    }

    /// Sets the position inside the file.
    #[must_use]
    pub fn with_span(mut self, span: Span) -> Diagnostic {
        self.span = Some(span);
        self
    }

    /// Adds one message template argument.
    #[must_use]
    pub fn with_arg(mut self, key: impl Into<String>, value: impl Into<String>) -> Diagnostic {
        self.args.insert(key.into(), value.into());
        self
    }

    /// The deterministic sort key: code, mod, file, span, message, severity, arguments.
    fn order(&self, other: &Diagnostic) -> Ordering {
        self.code
            .cmp(&other.code)
            .then_with(|| self.mod_idx.cmp(&other.mod_idx))
            .then_with(|| self.file.cmp(&other.file))
            .then_with(|| self.span.cmp(&other.span))
            .then_with(|| self.message.cmp(&other.message))
            .then_with(|| self.severity.cmp(&other.severity))
            .then_with(|| self.args.cmp(&other.args))
    }
}

/// Anything that accepts diagnostics. Producers take `&mut dyn DiagnosticSink` so engines stay
/// independent of how the caller stores them (no globals, invariant I-16).
pub trait DiagnosticSink {
    /// Records one diagnostic.
    fn emit(&mut self, diagnostic: Diagnostic);
}

/// A sink that drops everything, for callers that do not care.
#[derive(Debug, Default, Clone, Copy)]
pub struct DiscardSink;

impl DiagnosticSink for DiscardSink {
    fn emit(&mut self, _diagnostic: Diagnostic) {}
}

impl DiagnosticSink for Vec<Diagnostic> {
    fn emit(&mut self, diagnostic: Diagnostic) {
        self.push(diagnostic);
    }
}

/// The finished, sorted result of a [`DiagSink`], ready to be returned beside an operation result.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticSummary {
    /// The true number of diagnostics per code.
    pub counts: BTreeMap<DiagCode, u64>,
    /// The kept samples, sorted by code and then by the deterministic key.
    pub samples: Vec<Diagnostic>,
    /// True when at least one code had more diagnostics than kept samples.
    pub truncated: bool,
    /// Total errors.
    pub errors: u64,
    /// Total warnings.
    pub warnings: u64,
    /// Total infos.
    pub infos: u64,
    /// Total hints.
    pub hints: u64,
}

impl DiagnosticSummary {
    /// The total number of diagnostics over all codes.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// The count of one code, zero when absent.
    pub fn count(&self, code: &DiagCode) -> u64 {
        self.counts.get(code).copied().unwrap_or(0)
    }

    /// True when nothing was reported (the vanilla load test asserts this).
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }
}

/// The default sink: counts per code and keeps a bounded, deterministic sample set per code.
///
/// Each parallel worker owns one sink; the results are combined with [`DiagSink::merge`], in any
/// order, with an identical outcome.
#[derive(Debug, Clone)]
pub struct DiagSink {
    cap: usize,
    counts: BTreeMap<DiagCode, u64>,
    samples: BTreeMap<DiagCode, Vec<Diagnostic>>,
    by_severity: [u64; 4],
}

impl Default for DiagSink {
    fn default() -> Self {
        DiagSink::new()
    }
}

impl DiagSink {
    /// Creates a sink that keeps [`DEFAULT_SAMPLE_CAP`] samples per code.
    pub fn new() -> DiagSink {
        DiagSink::with_cap(DEFAULT_SAMPLE_CAP)
    }

    /// Creates a sink that keeps `cap` samples per code (zero keeps only counts).
    pub fn with_cap(cap: usize) -> DiagSink {
        DiagSink {
            cap,
            counts: BTreeMap::new(),
            samples: BTreeMap::new(),
            by_severity: [0; 4],
        }
    }

    /// The sample cap per code.
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Records one diagnostic: bumps its counters and keeps it when it sorts among the first `cap`
    /// samples of its code.
    pub fn push(&mut self, diagnostic: Diagnostic) {
        *self.counts.entry(diagnostic.code.clone()).or_insert(0) += 1;
        self.by_severity[diagnostic.severity.slot()] += 1;
        self.keep_sample(diagnostic);
    }

    fn keep_sample(&mut self, diagnostic: Diagnostic) {
        if self.cap == 0 {
            return;
        }
        let list = self.samples.entry(diagnostic.code.clone()).or_default();
        if list.len() >= self.cap {
            match list.last() {
                Some(last) if diagnostic.order(last) != Ordering::Less => return,
                _ => {}
            }
        }
        let at = list.partition_point(|existing| existing.order(&diagnostic) != Ordering::Greater);
        list.insert(at, diagnostic);
        list.truncate(self.cap);
    }

    /// Folds another sink into this one (counts add, samples are re-bounded). Use it to combine the
    /// per worker sinks of a parallel scan; the result does not depend on the merge order.
    pub fn merge(&mut self, other: DiagSink) {
        for (code, n) in other.counts {
            *self.counts.entry(code).or_insert(0) += n;
        }
        for (slot, n) in self.by_severity.iter_mut().zip(other.by_severity) {
            *slot += n;
        }
        for list in other.samples.into_values() {
            for sample in list {
                self.keep_sample(sample);
            }
        }
    }

    /// Merges many sinks into one with the given cap.
    pub fn merge_all<I: IntoIterator<Item = DiagSink>>(sinks: I, cap: usize) -> DiagSink {
        let mut out = DiagSink::with_cap(cap);
        for sink in sinks {
            out.merge(sink);
        }
        out
    }

    /// The true count of one code.
    pub fn count(&self, code: &DiagCode) -> u64 {
        self.counts.get(code).copied().unwrap_or(0)
    }

    /// The true total over all codes.
    pub fn total(&self) -> u64 {
        self.counts.values().sum()
    }

    /// The true count of one severity.
    pub fn count_severity(&self, severity: Severity) -> u64 {
        self.by_severity[severity.slot()]
    }

    /// The kept samples of one code, in deterministic order.
    pub fn samples(&self, code: &DiagCode) -> &[Diagnostic] {
        self.samples.get(code).map_or(&[], Vec::as_slice)
    }

    /// True when nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.counts.is_empty()
    }

    /// Produces the sorted summary. The sink is left intact so it can keep collecting.
    pub fn summary(&self) -> DiagnosticSummary {
        let mut samples = Vec::new();
        let mut truncated = false;
        for (code, list) in &self.samples {
            samples.extend(list.iter().cloned());
            if self.counts.get(code).copied().unwrap_or(0) > list.len() as u64 {
                truncated = true;
            }
        }
        if self.cap == 0 && !self.counts.is_empty() {
            truncated = true;
        }
        DiagnosticSummary {
            counts: self.counts.clone(),
            samples,
            truncated,
            errors: self.by_severity[0],
            warnings: self.by_severity[1],
            infos: self.by_severity[2],
            hints: self.by_severity[3],
        }
    }

    /// Consumes the sink and returns the summary.
    pub fn finish(self) -> DiagnosticSummary {
        self.summary()
    }
}

impl DiagnosticSink for DiagSink {
    fn emit(&mut self, diagnostic: Diagnostic) {
        self.push(diagnostic);
    }
}

impl Extend<Diagnostic> for DiagSink {
    fn extend<T: IntoIterator<Item = Diagnostic>>(&mut self, iter: T) {
        for d in iter {
            self.push(d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    const A: DiagCode = DiagCode::new("rs.alpha-thing");
    const B: DiagCode = DiagCode::new("rs.beta");

    fn diag(code: &DiagCode, mod_idx: u32, message: &str) -> Diagnostic {
        Diagnostic::new(code.clone(), Severity::Warning, message).with_mod(ModIdx(mod_idx))
    }

    #[rstest]
    #[case("xml.parse-error", true)]
    #[case("list.version-mismatch", true)]
    #[case("ce.lint-cep001", true)]
    #[case("a.b.c", true)]
    #[case("nodot", false)]
    #[case("Xml.parse", false)]
    #[case("xml.", false)]
    #[case(".xml", false)]
    #[case("xml.-bad", false)]
    #[case("xml.bad-", false)]
    #[case("xml.bad--double", false)]
    #[case("xml.under_score", false)]
    #[case("xml.1st", false)]
    #[case("", false)]
    fn code_grammar(#[case] code: &str, #[case] ok: bool) {
        assert_eq!(DiagCode::parse(code).is_ok(), ok, "{code}");
        assert_eq!(
            DiagCode::new(Box::leak(code.to_owned().into_boxed_str())).is_well_formed(),
            ok
        );
    }

    #[test]
    fn code_area_and_serde() {
        let c = DiagCode::parse("scan.about-missing").unwrap();
        assert_eq!(c.area(), "scan");
        assert_eq!(serde_json::to_string(&c).unwrap(), "\"scan.about-missing\"");
        assert!(serde_json::from_str::<DiagCode>("\"bad code\"").is_err());
    }

    #[test]
    fn severity_orders_from_most_serious() {
        let mut v = vec![
            Severity::Hint,
            Severity::Error,
            Severity::Info,
            Severity::Warning,
        ];
        v.sort();
        assert_eq!(v, Severity::ALL.to_vec());
        assert_eq!(
            serde_json::to_string(&Severity::Warning).unwrap(),
            "\"warning\""
        );
    }

    #[test]
    fn sink_counts_everything_and_caps_samples() {
        let mut sink = DiagSink::with_cap(3);
        for i in 0..10 {
            sink.push(diag(&A, i, "x"));
        }
        sink.push(diag(&B, 0, "y"));
        assert_eq!(sink.count(&A), 10);
        assert_eq!(sink.count(&B), 1);
        assert_eq!(sink.total(), 11);
        assert_eq!(sink.samples(&A).len(), 3);
        let summary = sink.summary();
        assert!(summary.truncated);
        assert_eq!(summary.total(), 11);
        assert_eq!(summary.warnings, 11);
        assert_eq!(summary.samples.len(), 4);
    }

    #[test]
    fn default_cap_is_one_hundred() {
        let mut sink = DiagSink::default();
        for i in 0..250 {
            sink.push(diag(&A, i, "x"));
        }
        assert_eq!(sink.cap(), 100);
        assert_eq!(sink.samples(&A).len(), 100);
        assert_eq!(sink.count(&A), 250);
    }

    #[test]
    fn kept_samples_are_the_first_by_sort_key_not_by_arrival() {
        let mut sink = DiagSink::with_cap(2);
        for i in [9u32, 7, 3, 8, 1, 5] {
            sink.push(diag(&A, i, "x"));
        }
        let kept: Vec<u32> = sink
            .samples(&A)
            .iter()
            .filter_map(|d| d.mod_idx)
            .map(|m| m.0)
            .collect();
        assert_eq!(kept, [1, 3]);
    }

    #[test]
    fn zero_cap_keeps_only_counts() {
        let mut sink = DiagSink::with_cap(0);
        sink.push(diag(&A, 1, "x"));
        let s = sink.summary();
        assert_eq!(s.count(&A), 1);
        assert!(s.samples.is_empty());
        assert!(s.truncated);
    }

    #[test]
    fn summary_is_empty_for_a_clean_run() {
        let sink = DiagSink::new();
        assert!(sink.is_empty());
        assert!(sink.summary().is_empty());
        assert!(!sink.summary().truncated);
    }

    #[test]
    fn merge_adds_counts_and_rebounds_samples() {
        let mut a = DiagSink::with_cap(2);
        let mut b = DiagSink::with_cap(2);
        for i in [5, 6] {
            a.push(diag(&A, i, "x"));
        }
        for i in [1, 2, 9] {
            b.push(diag(&A, i, "x"));
        }
        b.push(Diagnostic::new(B, Severity::Error, "e"));
        a.merge(b);
        assert_eq!(a.count(&A), 5);
        assert_eq!(a.count_severity(Severity::Error), 1);
        let kept: Vec<u32> = a
            .samples(&A)
            .iter()
            .filter_map(|d| d.mod_idx)
            .map(|m| m.0)
            .collect();
        assert_eq!(kept, [1, 2]);
    }

    #[test]
    fn diagnostics_serialise_without_empty_optionals() {
        let d = Diagnostic::new(A, Severity::Info, "m")
            .with_arg("k", "v")
            .with_span(Span::LineCol { line: 3, column: 4 });
        let json = serde_json::to_value(&d).unwrap();
        assert_eq!(json["code"], "rs.alpha-thing");
        assert_eq!(json["severity"], "info");
        assert_eq!(json["args"]["k"], "v");
        assert_eq!(json["span"]["kind"], "line-col");
        assert!(json.get("modIdx").is_none());
        let back: Diagnostic = serde_json::from_value(json).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn vec_and_discard_sinks_work_through_the_trait() {
        fn produce(sink: &mut dyn DiagnosticSink) {
            sink.emit(Diagnostic::new(A, Severity::Hint, "h"));
        }
        let mut v: Vec<Diagnostic> = Vec::new();
        produce(&mut v);
        assert_eq!(v.len(), 1);
        produce(&mut DiscardSink);
        let mut s = DiagSink::new();
        produce(&mut s);
        assert_eq!(s.count_severity(Severity::Hint), 1);
    }

    proptest! {
        #[test]
        fn merging_is_independent_of_order_and_partitioning(
            items in proptest::collection::vec((0u32..6, 0u32..12, "[a-c]{0,2}"), 0..60),
            split in 0usize..60,
        ) {
            let codes = [A, B];
            let all: Vec<Diagnostic> = items.iter().map(|(c, m, msg)| diag(&codes[(*c % 2) as usize], *m, msg)).collect();

            let mut single = DiagSink::with_cap(4);
            for d in &all { single.push(d.clone()); }

            let cut = split.min(all.len());
            let mut left = DiagSink::with_cap(4);
            let mut right = DiagSink::with_cap(4);
            for d in &all[..cut] { left.push(d.clone()); }
            for d in &all[cut..] { right.push(d.clone()); }

            let mut lr = DiagSink::with_cap(4);
            lr.merge(left.clone());
            lr.merge(right.clone());
            let mut rl = DiagSink::with_cap(4);
            rl.merge(right);
            rl.merge(left);

            prop_assert_eq!(single.summary(), lr.summary());
            prop_assert_eq!(single.summary(), rl.summary());
        }

        #[test]
        fn samples_never_exceed_the_cap_and_counts_are_exact(n in 0usize..300, cap in 0usize..20) {
            let mut sink = DiagSink::with_cap(cap);
            for i in 0..n { sink.push(diag(&A, (i % 17) as u32, "m")); }
            prop_assert_eq!(sink.count(&A), n as u64);
            prop_assert!(sink.samples(&A).len() <= cap);
            prop_assert_eq!(sink.samples(&A).len(), n.min(cap));
        }
    }
}
