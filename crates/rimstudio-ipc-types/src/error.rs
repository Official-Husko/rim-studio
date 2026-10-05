//! The error envelope every rejected call carries, and the registry of stable error codes.
//!
//! The envelope is `{ code, message, errorId, details }`. The `code` follows the grammar
//! `<area>.<kebab-name>` (the same grammar as diagnostic codes) and is API: the webview maps it to a
//! translated message and never matches on `message`. Source chains stay in the log, not on the wire.

use std::collections::hash_map::RandomState;
use std::fmt;
use std::hash::{BuildHasher, Hasher};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The serialised error of a failed command, stream or job.
///
/// `details` is a JSON object of structured parameters (never raw OS error text, never a path outside a
/// registered root). It is absent when the code needs no parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    /// Stable machine code, `<area>.<kebab-name>`; see [`codes`].
    pub code: String,
    /// Developer quality English fallback sentence; shown only when the UI has no entry for `code`.
    pub message: String,
    /// Random short id (`e-` and eight hex digits) that also appears in the log line of this failure.
    pub error_id: String,
    /// Structured parameters for message interpolation and recovery buttons; absent when there are none.
    /// Boxed so that `Result<T, ApiError>` stays small (the JSON is the same as for a plain map).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub details: Option<Box<Map<String, Value>>>,
}

impl ApiError {
    /// Builds an envelope with a freshly minted error id and no details.
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            error_id: mint_error_id(),
            details: None,
        }
    }

    /// Builds an envelope with a caller supplied error id (used when the id was minted earlier, and in tests).
    #[must_use]
    pub fn with_id(
        code: impl Into<String>,
        message: impl Into<String>,
        error_id: impl Into<String>,
    ) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            error_id: error_id.into(),
            details: None,
        }
    }

    /// Adds one structured detail; a later call with the same key replaces the value.
    #[must_use]
    pub fn detail(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.details
            .get_or_insert_with(Default::default)
            .insert(key.into(), value.into());
        self
    }

    /// The area of the code, the part before the first dot.
    #[must_use]
    pub fn area(&self) -> &str {
        self.code.split('.').next().unwrap_or("")
    }

    /// True when the code is in the [`codes::REGISTRY`].
    #[must_use]
    pub fn is_registered(&self) -> bool {
        codes::lookup(&self.code).is_some()
    }

    /// True for the cancellation code, which callers treat as a normal end.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.code == codes::JOB_CANCELLED
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({}): {}", self.code, self.error_id, self.message)
    }
}

impl std::error::Error for ApiError {}

static ERROR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Mints a random short error id of the form `e-8f3a21c9`.
///
/// Uses the per process random keys of the standard hasher mixed with a counter, so two ids minted in one
/// process never collide on the counter part and ids differ between runs.
#[must_use]
pub fn mint_error_id() -> String {
    let count = ERROR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(count);
    let bits = hasher.finish();
    format!("e-{:08x}", bits & 0xffff_ffff)
}

/// The registry of stable error codes.
pub mod codes {
    /// The recovery the UI offers for a code.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Recovery {
        /// Nothing useful the user can do beyond reading the message.
        None,
        /// Try the same call again.
        Retry,
        /// Run a library scan.
        Rescan,
        /// Open the application log.
        OpenLog,
        /// Open the settings page of the named section.
        OpenSettings(&'static str),
    }

    /// One registry row.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CodeInfo {
        /// The stable code.
        pub code: &'static str,
        /// The typed recovery.
        pub recovery: Recovery,
        /// True when the code is for developers and has no translated message.
        pub developer_only: bool,
    }

    macro_rules! registry {
        ($( $name:ident = $code:literal, $recovery:expr, $dev:expr; )*) => {
            $(
                #[doc = concat!("The code `", $code, "`.")]
                pub const $name: &str = $code;
            )*

            /// Every registered code, sorted by code text.
            pub const REGISTRY: &[CodeInfo] = &[
                $( CodeInfo { code: $code, recovery: $recovery, developer_only: $dev }, )*
            ];
        };
    }

    registry! {
        APP_INTERNAL_ERROR = "app.internal-error", Recovery::OpenLog, false;
        DEFS_INVALID_QUERY = "defs.invalid-query", Recovery::None, false;
        DEFS_SESSION_NOT_FOUND = "defs.session-not-found", Recovery::Retry, false;
        DESIGNER_APPLY_FAILED = "designer.apply-failed", Recovery::OpenLog, false;
        DESIGNER_CALIBRATION_UNAVAILABLE = "designer.calibration-unavailable", Recovery::OpenSettings("paths"), false;
        DESIGNER_DEFERRED = "designer.deferred", Recovery::None, false;
        DESIGNER_DRAFT_NEWER_SCHEMA = "designer.draft-newer-schema", Recovery::None, false;
        DESIGNER_DRAFT_NOT_FOUND = "designer.draft-not-found", Recovery::None, false;
        DESIGNER_INVALID_DRAFT = "designer.invalid-draft", Recovery::None, false;
        DESIGNER_PLAN_STALE = "designer.plan-stale", Recovery::Retry, false;
        DESIGNER_QUIZ_FINISHED = "designer.quiz-finished", Recovery::None, false;
        DESIGNER_QUIZ_WRONG_ANSWER = "designer.quiz-wrong-answer", Recovery::None, false;
        DESIGNER_REFERENCE_UNAVAILABLE = "designer.reference-unavailable", Recovery::OpenSettings("paths"), false;
        DETECT_FAILED = "detect.failed", Recovery::OpenLog, false;
        GAME_RUNNING = "game.running", Recovery::Retry, false;
        IO_NOT_A_DIRECTORY = "io.not-a-directory", Recovery::None, false;
        IO_NOT_FOUND = "io.not-found", Recovery::None, false;
        IO_PATH_OUTSIDE_ROOTS = "io.path-outside-roots", Recovery::OpenSettings("sources"), false;
        IO_PERMISSION_DENIED = "io.permission-denied", Recovery::None, false;
        IO_WRITE_FAILED = "io.write-failed", Recovery::OpenLog, false;
        IPC_INVALID_REQUEST = "ipc.invalid-request", Recovery::None, true;
        IPC_UNKNOWN_COMMAND = "ipc.unknown-command", Recovery::None, true;
        JOB_CANCELLED = "job.cancelled", Recovery::None, false;
        JOB_DUPLICATE_ID = "job.duplicate-id", Recovery::None, true;
        JOB_NOT_FOUND = "job.not-found", Recovery::None, true;
        JOB_PANICKED = "job.panicked", Recovery::OpenLog, false;
        LIBRARY_SCAN_FAILED = "library.scan-failed", Recovery::Rescan, false;
        PROJECT_FIX_JOURNAL_DAMAGED = "project.fix-journal-damaged", Recovery::None, false;
        PROJECT_FIX_NOT_FOUND = "project.fix-not-found", Recovery::None, false;
        PROJECT_FIX_UNDO_REFUSED = "project.fix-undo-refused", Recovery::None, false;
        PROJECT_INVALID_NAME = "project.invalid-name", Recovery::None, false;
        PROJECT_NOT_OPEN = "project.not-open", Recovery::None, false;
        PROJECT_PATH_OUTSIDE_ROOT = "project.path-outside-root", Recovery::None, false;
        SESSION_STALE = "session.stale", Recovery::Retry, false;
        SETTINGS_INVALID = "settings.invalid", Recovery::None, false;
        SETTINGS_NEWER_SCHEMA = "settings.newer-schema", Recovery::None, false;
        SETTINGS_REVISION_CONFLICT = "settings.revision-conflict", Recovery::Retry, false;
        SOURCES_INVALID_FOLDER = "sources.invalid-folder", Recovery::None, false;
        SOURCES_NOT_FOUND = "sources.not-found", Recovery::None, false;
        SOURCES_OVERLAP = "sources.overlap", Recovery::None, false;
    }

    /// Finds a registry row by code.
    #[must_use]
    pub fn lookup(code: &str) -> Option<&'static CodeInfo> {
        REGISTRY
            .binary_search_by(|row| row.code.cmp(code))
            .ok()
            .and_then(|i| REGISTRY.get(i))
    }

    /// True when the text follows the grammar `<area>.<kebab-name>`: lowercase ASCII, digits and single
    /// hyphens inside each dot separated part, at least two parts.
    #[must_use]
    pub fn is_well_formed(code: &str) -> bool {
        let mut parts = 0usize;
        for part in code.split('.') {
            parts += 1;
            let bytes = part.as_bytes();
            let Some(first) = bytes.first() else {
                return false;
            };
            if !first.is_ascii_lowercase() || bytes.last() == Some(&b'-') {
                return false;
            }
            let mut prev_hyphen = false;
            for &b in bytes {
                let hyphen = b == b'-';
                if !(b.is_ascii_lowercase() || b.is_ascii_digit() || hyphen)
                    || (hyphen && prev_hyphen)
                {
                    return false;
                }
                prev_hyphen = hyphen;
            }
        }
        parts >= 2
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_is_sorted_unique_and_well_formed() {
        let mut previous = "";
        for row in codes::REGISTRY {
            assert!(codes::is_well_formed(row.code), "{}", row.code);
            assert!(
                previous < row.code,
                "{previous} must sort before {}",
                row.code
            );
            previous = row.code;
        }
    }

    #[test]
    fn lookup_finds_every_registered_code_and_rejects_unknown() {
        for row in codes::REGISTRY {
            assert_eq!(codes::lookup(row.code).map(|r| r.code), Some(row.code));
        }
        assert!(codes::lookup("nope.not-a-code").is_none());
    }

    #[test]
    fn envelope_serialises_with_camel_case_and_omits_empty_details() {
        let error = ApiError::with_id("list.revision-conflict", "The list changed.", "e-8f3a21c9");
        let text = serde_json::to_string(&error).unwrap_or_default();
        assert_eq!(
            text,
            r#"{"code":"list.revision-conflict","message":"The list changed.","errorId":"e-8f3a21c9"}"#
        );
    }

    #[test]
    fn details_round_trip_and_replace_by_key() {
        let error = ApiError::with_id("settings.revision-conflict", "x", "e-00000001")
            .detail("expectedRev", 41)
            .detail("currentRev", 44)
            .detail("currentRev", 45);
        let text = serde_json::to_string(&error).unwrap_or_default();
        let back: Result<ApiError, _> = serde_json::from_str(&text);
        assert_eq!(back.ok(), Some(error));
        assert!(text.contains(r#""currentRev":45"#));
    }

    #[test]
    fn minted_ids_have_the_documented_shape_and_differ() {
        let a = mint_error_id();
        let b = mint_error_id();
        assert_eq!(a.len(), 10);
        assert!(a.starts_with("e-"));
        assert!(a[2..].bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn cancellation_and_registration_helpers() {
        let error = ApiError::new(codes::JOB_CANCELLED, "Cancelled.");
        assert!(error.is_cancelled());
        assert!(error.is_registered());
        assert_eq!(error.area(), "job");
        assert!(!ApiError::new("zzz.unknown", "x").is_registered());
    }
}
