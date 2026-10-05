//! The stable diagnostic codes of the def engine.
//!
//! The research prototype used snake case codes such as `patch_failed`. At the API edge they are
//! mapped to `defs.<kebab-name>` ([`from_legacy`] performs the mapping, which the research vectors
//! use). A leading `def_` or `defs_` of the legacy name is dropped (`def_unknown_type` becomes
//! `defs.unknown-type`). The first 18 constants are the codes of the research note; the rest are
//! problems the prototype counted under finer names.

use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
use rimstudio_core::ids::{FileId, ModIdx};

/// A top-level patch operation returned false.
pub const PATCH_FAILED: DiagCode = DiagCode::new("defs.patch-failed");
/// A patch operation raised an exception (or panicked).
pub const PATCH_EXCEPTION: DiagCode = DiagCode::new("defs.patch-exception");
/// A patch `Class` names no known operation class.
pub const PATCH_UNKNOWN_CLASS: DiagCode = DiagCode::new("defs.patch-unknown-class");
/// The base `PatchOperation` was applied (missing or unknown class), which always fails.
pub const PATCH_BASE_CLASS: DiagCode = DiagCode::new("defs.patch-base-class");
/// An operation element has a child the class has no field for.
pub const PATCH_UNKNOWN_FIELD: DiagCode = DiagCode::new("defs.patch-unknown-field");
/// A `ParentName` that no registered node answers to.
pub const INHERIT_MISSING_PARENT: DiagCode = DiagCode::new("defs.inherit-missing-parent");
/// A node is part of, or below, an inheritance cycle.
pub const INHERIT_CYCLE: DiagCode = DiagCode::new("defs.inherit-cycle");
/// The resolved node of a node with a `ParentName` is unavailable.
pub const INHERIT_NOT_RESOLVED: DiagCode = DiagCode::new("defs.inherit-not-resolved");
/// A `Name` used twice inside one mod.
pub const INHERIT_DUPLICATE_NAME: DiagCode = DiagCode::new("defs.inherit-duplicate-name");
/// A non-list element name used twice inside one node.
pub const INHERIT_DUPLICATE_NODE_NAME: DiagCode = DiagCode::new("defs.inherit-duplicate-node-name");
/// A node whose element name or `Class` is no def type.
pub const UNKNOWN_TYPE: DiagCode = DiagCode::new("defs.unknown-type");
/// Two defs of one type with one `defName` inside one mod.
pub const DUPLICATE_IN_MOD: DiagCode = DiagCode::new("defs.duplicate-in-mod");
/// A defs file whose root element is not `Defs`.
pub const BAD_ROOT: DiagCode = DiagCode::new("defs.bad-root");
/// A file could not be read as XML.
pub const XML_PARSE_ERROR: DiagCode = DiagCode::new("defs.xml-parse-error");
/// A defs file has no document (it was skipped).
pub const UNKNOWN_PARSE_FAILURE: DiagCode = DiagCode::new("defs.unknown-parse-failure");
/// A patch file has no document (the game would throw while building the list).
pub const PATCH_FILE_UNREADABLE: DiagCode = DiagCode::new("defs.patch-file-unreadable");
/// A patch field name matched only ignoring case.
pub const PATCH_FIELD_CASE_MISMATCH: DiagCode = DiagCode::new("defs.patch-field-case-mismatch");
/// A settings conditional named a setting that does not exist.
pub const PATCH_SETTING_MISSING: DiagCode = DiagCode::new("defs.patch-setting-missing");

/// A list field holds an element that is not `li`.
pub const PATCH_LIST_ITEM_NOT_LI: DiagCode = DiagCode::new("defs.patch-list-item-not-li");
/// An operation defines the same field twice.
pub const PATCH_DUPLICATE_FIELD: DiagCode = DiagCode::new("defs.patch-duplicate-field");
/// A field value cannot be parsed (an invalid `success` or `order`).
pub const PATCH_FIELD_PARSE_ERROR: DiagCode = DiagCode::new("defs.patch-field-parse-error");
/// A patch file whose root element is not `Patch`.
pub const PATCH_BAD_ROOT: DiagCode = DiagCode::new("defs.patch-bad-root");
/// A patch file element that is not `Operation`.
pub const PATCH_BAD_ELEMENT: DiagCode = DiagCode::new("defs.patch-bad-element");
/// A def node without a `defName` was given a generated name (information: the game names some
/// types in code, for example a song from its clip path).
pub const UNNAMED_DEF: DiagCode = DiagCode::new("defs.unnamed-def");
/// A node could not be imported into the unified document.
pub const IMPORT_FAILED: DiagCode = DiagCode::new("defs.import-failed");

/// Every code of this crate, in a fixed order.
pub const ALL: [DiagCode; 25] = [
    PATCH_FAILED,
    PATCH_EXCEPTION,
    PATCH_UNKNOWN_CLASS,
    PATCH_BASE_CLASS,
    PATCH_UNKNOWN_FIELD,
    INHERIT_MISSING_PARENT,
    INHERIT_CYCLE,
    INHERIT_NOT_RESOLVED,
    INHERIT_DUPLICATE_NAME,
    INHERIT_DUPLICATE_NODE_NAME,
    UNKNOWN_TYPE,
    DUPLICATE_IN_MOD,
    BAD_ROOT,
    XML_PARSE_ERROR,
    UNKNOWN_PARSE_FAILURE,
    PATCH_FILE_UNREADABLE,
    PATCH_FIELD_CASE_MISMATCH,
    PATCH_SETTING_MISSING,
    PATCH_LIST_ITEM_NOT_LI,
    PATCH_DUPLICATE_FIELD,
    PATCH_FIELD_PARSE_ERROR,
    PATCH_BAD_ROOT,
    PATCH_BAD_ELEMENT,
    UNNAMED_DEF,
    IMPORT_FAILED,
];

/// Maps a legacy (research prototype) snake case code to its stable code.
///
/// Returns `None` for names this crate does not define.
#[must_use]
pub fn from_legacy(legacy: &str) -> Option<DiagCode> {
    let code = match legacy {
        "patch_failed" => PATCH_FAILED,
        "patch_exception" => PATCH_EXCEPTION,
        "patch_unknown_class" => PATCH_UNKNOWN_CLASS,
        "patch_base_class" => PATCH_BASE_CLASS,
        "patch_unknown_field" => PATCH_UNKNOWN_FIELD,
        "inherit_missing_parent" => INHERIT_MISSING_PARENT,
        "inherit_cycle" => INHERIT_CYCLE,
        "inherit_not_resolved" => INHERIT_NOT_RESOLVED,
        "inherit_duplicate_name" => INHERIT_DUPLICATE_NAME,
        "inherit_duplicate_node_name" => INHERIT_DUPLICATE_NODE_NAME,
        "def_unknown_type" => UNKNOWN_TYPE,
        "def_duplicate_in_mod" => DUPLICATE_IN_MOD,
        "defs_bad_root" => BAD_ROOT,
        "xml_parse_error" => XML_PARSE_ERROR,
        "defs_unknown_parse_failure" => UNKNOWN_PARSE_FAILURE,
        "patch_file_unreadable" => PATCH_FILE_UNREADABLE,
        "patch_field_case_mismatch" => PATCH_FIELD_CASE_MISMATCH,
        "patch_setting_missing" => PATCH_SETTING_MISSING,
        "patch_list_item_not_li" => PATCH_LIST_ITEM_NOT_LI,
        "patch_duplicate_field" => PATCH_DUPLICATE_FIELD,
        "patch_field_parse_error" => PATCH_FIELD_PARSE_ERROR,
        "patch_bad_root" => PATCH_BAD_ROOT,
        "patch_bad_element" => PATCH_BAD_ELEMENT,
        _ => return None,
    };
    Some(code)
}

/// The place a diagnostic points to: an optional mod and an optional file.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Site {
    /// The mod the diagnostic concerns.
    pub mod_idx: Option<ModIdx>,
    /// The file the diagnostic concerns.
    pub file: Option<FileId>,
}

impl Site {
    /// A site from an optional mod and file.
    #[must_use]
    pub fn new(mod_idx: Option<ModIdx>, file: Option<FileId>) -> Self {
        Self { mod_idx, file }
    }

    /// Builds a diagnostic located at this site.
    #[must_use]
    pub fn diag(
        self,
        code: DiagCode,
        severity: Severity,
        message: impl Into<String>,
    ) -> Diagnostic {
        let mut d = Diagnostic::new(code, severity, message);
        if let Some(m) = self.mod_idx {
            d = d.with_mod(m);
        }
        if let Some(f) = self.file {
            d = d.with_file(f);
        }
        d
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn every_code_is_well_formed_and_unique() {
        let mut seen = BTreeSet::new();
        for code in ALL {
            assert!(code.is_well_formed(), "{}", code.as_str());
            assert_eq!(code.area(), "defs");
            assert!(seen.insert(code.as_str().to_owned()), "{}", code.as_str());
        }
    }

    #[test]
    fn legacy_names_map_by_rule() {
        assert_eq!(from_legacy("patch_failed"), Some(PATCH_FAILED));
        assert_eq!(from_legacy("def_unknown_type"), Some(UNKNOWN_TYPE));
        assert_eq!(from_legacy("defs_bad_root"), Some(BAD_ROOT));
        assert_eq!(from_legacy("nope"), None);
    }

    #[test]
    fn legacy_mapping_covers_the_research_codes() {
        let names = [
            "patch_failed",
            "patch_exception",
            "patch_unknown_class",
            "patch_base_class",
            "patch_unknown_field",
            "inherit_missing_parent",
            "inherit_cycle",
            "inherit_not_resolved",
            "inherit_duplicate_name",
            "inherit_duplicate_node_name",
            "def_unknown_type",
            "def_duplicate_in_mod",
            "defs_bad_root",
            "xml_parse_error",
            "defs_unknown_parse_failure",
            "patch_file_unreadable",
            "patch_field_case_mismatch",
            "patch_setting_missing",
        ];
        for n in names {
            assert!(from_legacy(n).is_some(), "{n}");
        }
    }
}
