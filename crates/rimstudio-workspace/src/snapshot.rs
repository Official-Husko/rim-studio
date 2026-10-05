//! Immutable def database snapshots and the views read from them.
//!
//! A [`Snapshot`] is the result of one def engine load: the merged, patched and inheritance
//! resolved defs, their databases, the patch report and the diagnostics, plus the pack and file
//! tables needed to turn the engine's session handles back into package ids and paths. Snapshots
//! are immutable and cheap to share (`Arc`); the session swaps in a new one on rebuild and tools
//! keep the one they started with.
//!
//! [`Snapshot::resolve_def`] answers "what is this def, after everything": the resolved node and
//! its provenance (defining file, inheritance chain, patch operations that touched it).

use std::sync::Arc;

use camino::Utf8PathBuf;
use rimstudio_core::diag::DiagnosticSummary;
use rimstudio_core::ids::FileId;
use rimstudio_core::tree::Node;
use rimstudio_defs::apply::PatchReport;
use rimstudio_defs::build::DefDatabases;
use rimstudio_defs::{DefRecord, LoadStats, LoadTimings, ModOrder};
use serde::{Deserialize, Serialize};

use crate::refset::PackKind;

/// A pack as the views name it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackRef {
    /// The position of the pack in the reference set (the engine's mod handle).
    pub index: usize,
    /// The package id as declared.
    pub package_id: String,
    /// The display name.
    pub name: String,
    /// What the pack is.
    pub kind: PackKind,
}

/// A source file of the session, by handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRef {
    /// The handle (the engine's `FileId`, an index into the session's file table).
    pub id: u32,
    /// The pack the file belongs to.
    pub pack: usize,
    /// The path relative to its load folder, `Defs/...` or `Patches/...`.
    pub relative: String,
    /// The absolute path.
    pub path: Utf8PathBuf,
}

/// Identifies a def: its type, its name and, to pick one copy among overrides, its pack.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefRef {
    /// The def type: the element name, the short class name or the full class name (case
    /// insensitive for the last two).
    pub def_type: String,
    /// The `defName`.
    pub def_name: String,
    /// The package id of the pack that owns the copy; `None` means the winning copy.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pack: Option<String>,
}

impl DefRef {
    /// A reference to the winning copy of a def.
    #[must_use]
    pub fn new(def_type: impl Into<String>, def_name: impl Into<String>) -> Self {
        DefRef {
            def_type: def_type.into(),
            def_name: def_name.into(),
            pack: None,
        }
    }

    /// Restricts the reference to the copy defined by one pack.
    #[must_use]
    pub fn in_pack(mut self, package_id: impl Into<String>) -> Self {
        self.pack = Some(package_id.into());
        self
    }
}

/// One ancestor of a def.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParentInfo {
    /// The pack that declared the parent; `None` for a parent created by a patch.
    pub pack: Option<PackRef>,
    /// The parent's `Name`.
    pub name: String,
}

/// A patch operation that touched a def.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PatchInfo {
    /// The pack whose patch list holds the operation.
    pub pack: Option<PackRef>,
    /// The patch file.
    pub file: Option<FileRef>,
    /// The position of the top level operation in the pack's patch list.
    pub op_index: u32,
    /// The operation class.
    pub class: String,
    /// The operation's XPath.
    pub xpath: Option<String>,
    /// `Class(xpath)` as one line.
    pub description: String,
}

/// Where a resolved def comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefProvenance {
    /// The full type name from the type table.
    pub type_name: String,
    /// The element name as written.
    pub tag: String,
    /// The `defName`.
    pub def_name: String,
    /// The pack that defines the def; `None` for a def created by a patch.
    pub pack: Option<PackRef>,
    /// The defining file.
    pub file: Option<FileRef>,
    /// The inheritance chain, nearest parent first.
    pub parents: Vec<ParentInfo>,
    /// The patch operations that touched the def, in application order.
    pub patched_by: Vec<PatchInfo>,
}

/// A def after patches and inheritance, with its provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedDefView {
    /// Where the def comes from.
    pub provenance: DefProvenance,
    /// The resolved node tree.
    pub node: Node,
}

/// The result of one def engine load with the tables to read it.
#[derive(Debug)]
pub struct Snapshot {
    /// The revision number: 1 for the first build of a session, plus one per rebuild.
    pub revision: u64,
    /// The per type databases.
    pub databases: Arc<DefDatabases>,
    /// Every def in merged document order (overridden copies included).
    pub defs: Arc<Vec<DefRecord>>,
    /// The engine's counters.
    pub stats: LoadStats,
    /// Time per engine stage.
    pub timings: LoadTimings,
    /// The engine's diagnostics (patch failures, inheritance problems, unknown types).
    pub diagnostics: DiagnosticSummary,
    /// One event per top level patch operation.
    pub patch_report: PatchReport,
    /// The packs in load order as the engine saw them.
    pub order: ModOrder,
    /// The packs of the reference set, by index.
    pub packs: Vec<PackRef>,
    /// The files of the session, by handle.
    pub files: Vec<FileRef>,
}

impl Snapshot {
    /// The pack with this index.
    #[must_use]
    pub fn pack(&self, index: usize) -> Option<&PackRef> {
        self.packs.get(index)
    }

    /// The file with this handle.
    #[must_use]
    pub fn file(&self, id: FileId) -> Option<&FileRef> {
        self.files.get(id.index())
    }

    fn pack_ref(&self, idx: Option<rimstudio_core::ids::ModIdx>) -> Option<PackRef> {
        idx.and_then(|m| self.packs.get(m.index())).cloned()
    }

    fn matches_type(record: &DefRecord, wanted: &str) -> bool {
        let wanted = wanted.trim();
        record.tag == wanted
            || record.type_name.eq_ignore_ascii_case(wanted)
            || record
                .type_name
                .rsplit(['.', '+'])
                .next()
                .is_some_and(|short| short.eq_ignore_ascii_case(wanted))
    }

    fn find(&self, def: &DefRef) -> Option<&DefRecord> {
        match &def.pack {
            None => self
                .databases
                .get(&def.def_type, &def.def_name)
                .or_else(|| self.scan(def, None)),
            Some(pack) => self.scan(def, Some(pack)),
        }
    }

    fn scan(&self, def: &DefRef, pack: Option<&str>) -> Option<&DefRecord> {
        self.defs.iter().rev().find(|d| {
            d.def_name == def.def_name
                && Self::matches_type(d, &def.def_type)
                && pack.is_none_or(|p| {
                    self.pack_ref(d.mod_idx())
                        .is_some_and(|r| r.package_id.eq_ignore_ascii_case(p.trim()))
                })
        })
    }

    /// The raw engine record of a def.
    #[must_use]
    pub fn record(&self, def: &DefRef) -> Option<&DefRecord> {
        self.find(def)
    }

    /// The def after patches and inheritance with its provenance, `None` when the snapshot has no
    /// such def (an abstract node is not a def).
    #[must_use]
    pub fn resolve_def(&self, def: &DefRef) -> Option<ResolvedDefView> {
        let record = self.find(def)?;
        let file_of = |f: FileId| self.file(f).cloned();
        let provenance = DefProvenance {
            type_name: record.type_name.clone(),
            tag: record.tag.clone(),
            def_name: record.def_name.clone(),
            pack: self.pack_ref(record.mod_idx()),
            file: record.origin.and_then(|o| file_of(o.file)),
            parents: record
                .parents
                .iter()
                .map(|p| ParentInfo {
                    pack: self.pack_ref(p.mod_idx),
                    name: p.name.clone(),
                })
                .collect(),
            patched_by: record
                .patched_by
                .iter()
                .map(|p| PatchInfo {
                    pack: self.pack_ref(Some(p.mod_idx)),
                    file: p.file.and_then(file_of),
                    op_index: p.op_index,
                    class: p.class.clone(),
                    xpath: p.xpath.clone(),
                    description: p.description.clone(),
                })
                .collect(),
        };
        Some(ResolvedDefView {
            provenance,
            node: record.node.clone(),
        })
    }
}
