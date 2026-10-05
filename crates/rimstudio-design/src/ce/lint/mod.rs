//! Lint of Combat Extended patch files ("check my CE patch"): the rules CEP001 to CEP022.
//!
//! [`run`] checks parsed patch files against the rules of the specification (section 8 of the Combat
//! Extended patching document). The structural rules always run. The rules that need Combat Extended data
//! (CEP010, CEP013, CEP014, CEP015, CEP016) report `ce.not-checked` once when the model is absent or when
//! the table they need was not supplied, instead of guessing.
//!
//! Every finding is a [`Diagnostic`] with the stable code `ce.cep<nnn>-<name>` ([`codes`]), and the
//! arguments `ruleId` (`CEP013`), `path` (the file, when known) and `field` (a pointer such as
//! `/Patch/Operation[3]/AmmoUser/ammoSet`, one based positions). The order is deterministic: file by file in
//! the order given, operation by operation, rule by rule.
//!
//! The lint has no IO and no XML dependency: the caller parses the files with the boundary crate and hands
//! over node trees (a file that failed to parse is a [`LintFile`] with its error, CEP017).

pub mod codes;
mod rules;
pub mod xpath;

use std::collections::{BTreeMap, BTreeSet};

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::Node;
use serde::{Deserialize, Serialize};

use crate::ce::reader::CeModel;

pub use codes::{OTHER, REGISTRY, rule_id};

/// What the lint knows about the project besides the patch files themselves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LintContext {
    /// The path of each file given to [`run`], relative to the mod root with `/` separators (parallel to the
    /// files; missing entries mean the path is unknown, which disables the path rules for that file).
    pub paths: Vec<String>,
    /// The parsed `LoadFolders.xml` of the mod, `None` when it has none.
    pub load_folders: Option<Node>,
    /// The game version whose block of `LoadFolders.xml` applies, `1.6` by default.
    pub game_version: String,
    /// Def names that exist in the project, the patched mod and vanilla (ammo sets and projectiles that
    /// Combat Extended itself does not define).
    pub known_defs: BTreeSet<String>,
    /// The type names of the installed Combat Extended assemblies (CEP010). `None` when not available.
    pub ce_types: Option<BTreeSet<String>>,
    /// The known field names by section: `Properties`, `AmmoUser`, `FireModes`, `ToolCE` (CEP015). `None`
    /// when not available.
    pub known_fields: Option<BTreeMap<String, BTreeSet<String>>>,
}

impl Default for LintContext {
    fn default() -> Self {
        Self {
            paths: Vec::new(),
            load_folders: None,
            game_version: "1.6".to_owned(),
            known_defs: BTreeSet::new(),
            ce_types: None,
            known_fields: None,
        }
    }
}

/// One file given to the lint.
#[derive(Debug, Clone, PartialEq)]
pub struct LintFile {
    /// The path relative to the mod root, when known.
    pub path: Option<String>,
    /// The parsed root element, `None` when the file did not parse.
    pub root: Option<Node>,
    /// Why the file did not parse.
    pub parse_error: Option<String>,
}

impl LintFile {
    /// A parsed file.
    #[must_use]
    pub fn parsed(path: Option<String>, root: Node) -> Self {
        Self {
            path,
            root: Some(root),
            parse_error: None,
        }
    }

    /// A file that failed to parse (CEP017).
    #[must_use]
    pub fn failed(path: Option<String>, error: impl Into<String>) -> Self {
        Self {
            path,
            root: None,
            parse_error: Some(error.into()),
        }
    }
}

/// Checks parsed patch files. The path of file `i` is `ctx.paths[i]` when given. See the module
/// documentation for the rules and the order of the result.
#[must_use]
pub fn run(files: &[Node], model: &CeModel, ctx: &LintContext) -> Vec<Diagnostic> {
    let files: Vec<LintFile> = files
        .iter()
        .enumerate()
        .map(|(i, root)| LintFile::parsed(ctx.paths.get(i).cloned(), root.clone()))
        .collect();
    run_files(&files, model, ctx)
}

/// Checks patch files, some of which may have failed to parse.
#[must_use]
pub fn run_files(files: &[LintFile], model: &CeModel, ctx: &LintContext) -> Vec<Diagnostic> {
    rules::run(files, model, ctx)
}
