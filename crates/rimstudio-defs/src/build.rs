//! Def creation, the per type databases and the one entry point, [`load`].
//!
//! After inheritance every top-level node is checked in the game's order: `MayRequire` and
//! `MayRequireAnyOf` (an empty value never matches), `Abstract` (read from the original node and
//! never inherited), then the type from the element name or the resolved node's `Class`
//! ([`TypeTable::lookup`]). What survives becomes a [`DefRecord`] in merged document order.
//!
//! [`DefDatabases`] reproduces `DefDatabase<T>.AddAllInMods` for every def type: Core first, then
//! the other mods in load order, then the defs patches created; a def with a name that an earlier mod
//! already contributed replaces it and moves to the end; a second def with a name inside one mod is
//! skipped. A database contains the defs of its type and of every subclass. Overrides and
//! duplicates are exposed as data.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Instant;

use rayon::prelude::*;
use rimstudio_core::diag::{DiagSink, DiagnosticSummary, Severity};
use rimstudio_core::ids::ModIdx;
use rimstudio_core::mods::ActiveSet;
use rimstudio_core::tree::Node;
use rustc_hash::{FxHashMap, FxHashSet};
use serde::Serialize;

use crate::apply::{PatchContext, PatchReport, apply_patches};
use crate::custom_ops::CustomRegistry;
use crate::diag_codes::{self, Site};
use crate::error::{DefsError, DefsResult};
use crate::inherit::{InheritConfig, ResolvedDoc, ResolvedNode, resolve_inheritance};
use crate::merge::merge;
use crate::patch_ops::{ParseEnv, PatchOp, parse_patch_file};
use crate::provenance::{ModEntry, ModOrder};
use crate::type_table::TypeTable;
use crate::types::{DefFile, DefRecord, FileContent, LoadStats, PatchFile};

/// The result of [`build_defs_with`].
#[derive(Debug, Clone, Default)]
pub struct BuildOutput {
    /// The defs in merged document order.
    pub defs: Vec<DefRecord>,
    /// Nodes skipped because `Abstract` is true.
    pub abstract_nodes: u32,
    /// Nodes skipped because `MayRequire` or `MayRequireAnyOf` was not met.
    pub may_require_skipped: u32,
    /// Nodes dropped because their type is no def type.
    pub unknown_type: u32,
}

fn may_require_ok(n: &ResolvedNode, active: &ActiveSet) -> bool {
    if let Some(mr) = &n.may_require {
        let ids: Vec<&str> = mr.split(',').collect();
        if !active.all_active(&ids) {
            return false;
        }
    }
    if let Some(any) = &n.may_require_any_of {
        let ids: Vec<&str> = any.split(',').collect();
        if !active.any_active(&ids) {
            return false;
        }
    }
    true
}

/// Creates the defs of a resolved document, consuming it, and records problems in `sink`.
pub fn build_defs_with(
    resolved: ResolvedDoc,
    types: &TypeTable,
    active: &ActiveSet,
    sink: &mut DiagSink,
) -> BuildOutput {
    let mut out = BuildOutput::default();
    for n in resolved.nodes {
        if !may_require_ok(&n, active) {
            out.may_require_skipped = out.may_require_skipped.saturating_add(1);
            continue;
        }
        if n.abstract_attr
            .as_deref()
            .is_some_and(|a| a.eq_ignore_ascii_case("true"))
        {
            out.abstract_nodes = out.abstract_nodes.saturating_add(1);
            continue;
        }
        let site = Site::new(n.origin.map(|o| o.mod_idx), n.origin.map(|o| o.file));
        if n.parent_name.is_some() && !n.resolved {
            sink.push(site.diag(
                diag_codes::INHERIT_NOT_RESOLVED,
                Severity::Error,
                format!(
                    "Tried to get resolved node for node \"{}\" which uses a ParentName attribute, \
                     but it was never registered or failed to resolve.",
                    n.tag
                ),
            ));
        }
        let type_name = n.node.attr("Class").unwrap_or(&n.tag);
        let Some(full) = types.lookup(type_name) else {
            out.unknown_type = out.unknown_type.saturating_add(1);
            sink.push(site.diag(
                diag_codes::UNKNOWN_TYPE,
                Severity::Error,
                format!("Type {type_name} is not a Def type or could not be found"),
            ));
            continue;
        };
        let full = full.to_owned();
        let seq = u32::try_from(out.defs.len()).unwrap_or(u32::MAX);
        let def_name = match n
            .node
            .children_named("defName")
            .last()
            .map(Node::text_content)
        {
            Some(name) => name,
            None => {
                // The game names some types in code (a song from its clip path) and reports an
                // error for the others, which this XML level engine cannot tell apart, so the
                // problem is information only.
                sink.push(site.diag(
                    diag_codes::UNNAMED_DEF,
                    Severity::Info,
                    format!(
                        "A {} def has no defName and was given a generated name",
                        n.tag
                    ),
                ));
                format!("UnnamedDef{seq}")
            }
        };
        out.defs.push(DefRecord {
            seq,
            tag: n.tag,
            type_name: full,
            def_name,
            origin: n.origin,
            node: n.node,
            parents: n.parents,
            patched_by: n.patched_by,
        });
    }
    out
}

/// Creates the defs of a resolved document. Problems are discarded; use [`build_defs_with`] to see
/// them.
#[must_use]
pub fn build_defs(resolved: &ResolvedDoc, types: &TypeTable, active: &ActiveSet) -> Vec<DefRecord> {
    let mut sink = DiagSink::with_cap(0);
    build_defs_with(resolved.clone(), types, active, &mut sink).defs
}

/// One `DefDatabase<T>`.
#[derive(Debug, Clone, Default)]
struct Database {
    /// Indices into the defs, in database order.
    entries: Vec<u32>,
    by_name: FxHashMap<String, u32>,
    overridden: Vec<(u32, u32)>,
    skipped: Vec<(u32, u32)>,
}

/// A def that replaced an earlier def of the same name in one database.
#[derive(Debug, Clone, Copy)]
pub struct Override<'a> {
    /// The full name of the database type.
    pub db_type: &'a str,
    /// The def that was replaced.
    pub old: &'a DefRecord,
    /// The def that replaced it.
    pub new: &'a DefRecord,
}

impl Override<'_> {
    /// True when the replacing def has another type than the replaced one (a vanilla thing turned
    /// into a subclass by a mod, for example).
    #[must_use]
    pub fn type_changed(&self) -> bool {
        self.old.type_name != self.new.type_name
    }
}

/// A def skipped because its mod already contributed a def with the same name to the database.
#[derive(Debug, Clone, Copy)]
pub struct Duplicate<'a> {
    /// The full name of the database type.
    pub db_type: &'a str,
    /// The def that stays.
    pub kept: &'a DefRecord,
    /// The def that was skipped.
    pub skipped: &'a DefRecord,
}

/// A read only view of one database.
#[derive(Debug, Clone, Copy)]
pub struct DatabaseView<'a> {
    db_type: &'a str,
    defs: &'a [DefRecord],
    db: &'a Database,
}

impl<'a> DatabaseView<'a> {
    /// The full name of the database type.
    #[must_use]
    pub fn type_name(&self) -> &'a str {
        self.db_type
    }

    /// The defs in database order.
    pub fn iter(&self) -> impl Iterator<Item = &'a DefRecord> + use<'a> {
        let defs = self.defs;
        self.db
            .entries
            .iter()
            .filter_map(move |&i| defs.get(i as usize))
    }

    /// The def with the given name.
    #[must_use]
    pub fn get(&self, def_name: &str) -> Option<&'a DefRecord> {
        self.db
            .by_name
            .get(def_name)
            .and_then(|&i| self.defs.get(i as usize))
    }

    /// Number of defs.
    #[must_use]
    pub fn len(&self) -> usize {
        self.db.entries.len()
    }

    /// True when the database is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.db.entries.is_empty()
    }

    /// The defs in database order, by name.
    #[must_use]
    pub fn names(&self) -> Vec<&'a str> {
        self.iter().map(|d| d.def_name.as_str()).collect()
    }
}

/// The def databases of a load: one per def type that has a def, subclasses included.
#[derive(Debug, Clone)]
pub struct DefDatabases {
    defs: Arc<Vec<DefRecord>>,
    types: Arc<TypeTable>,
    dbs: BTreeMap<String, Database>,
    diagnostics: DiagSink,
}

impl DefDatabases {
    /// Builds the databases from defs in merged document order.
    ///
    /// Clones `defs`; use [`DefDatabases::build_shared`] to share them.
    #[must_use]
    pub fn build(defs: &[DefRecord], types: &TypeTable, order: &ModOrder) -> Self {
        Self::build_shared(Arc::new(defs.to_vec()), Arc::new(types.clone()), order)
    }

    /// Builds the databases over shared defs and a shared type table.
    #[must_use]
    pub fn build_shared(
        defs: Arc<Vec<DefRecord>>,
        types: Arc<TypeTable>,
        order: &ModOrder,
    ) -> Self {
        let mut members: BTreeMap<usize, Vec<u32>> = BTreeMap::new();
        for (i, d) in defs.iter().enumerate() {
            let Some(tid) = types.type_id(&d.type_name) else {
                continue;
            };
            let Ok(idx) = u32::try_from(i) else { continue };
            for &a in types.ancestor_ids(tid) {
                let a = a as usize;
                if types.name_at(a) != Some(types.root()) {
                    members.entry(a).or_default().push(idx);
                }
            }
        }
        let mod_order = order.database_order();
        let mut diagnostics = DiagSink::new();
        let mut dbs = BTreeMap::new();
        for (tid, list) in members {
            let Some(name) = types.name_at(tid) else {
                continue;
            };
            let db = build_database(name, &list, &defs, &mod_order, &mut diagnostics);
            dbs.insert(name.to_owned(), db);
        }
        Self {
            defs,
            types,
            dbs,
            diagnostics,
        }
    }

    /// The defs of the load.
    #[must_use]
    pub fn defs(&self) -> &[DefRecord] {
        &self.defs
    }

    /// The type table the databases were built with.
    #[must_use]
    pub fn types(&self) -> &TypeTable {
        &self.types
    }

    /// Diagnostics found while building (`defs.duplicate-in-mod`).
    #[must_use]
    pub fn diagnostics(&self) -> &DiagSink {
        &self.diagnostics
    }

    /// The full names of the database types that exist, sorted.
    #[must_use]
    pub fn type_names(&self) -> Vec<&str> {
        self.dbs.keys().map(String::as_str).collect()
    }

    /// The database for a type given by short or full name (resolved like a `Class` attribute).
    ///
    /// # Errors
    /// [`DefsError::UnknownType`] when the name is no def type, [`DefsError::RootType`] for the root
    /// type, which has no database. A known type without defs gives an empty view.
    pub fn database(&self, db_type: &str) -> DefsResult<DatabaseView<'_>> {
        let full = self
            .types
            .lookup(db_type)
            .ok_or_else(|| DefsError::UnknownType {
                name: db_type.to_owned(),
            })?;
        if full == self.types.root() {
            return Err(DefsError::RootType {
                name: full.to_owned(),
            });
        }
        static EMPTY: std::sync::OnceLock<Database> = std::sync::OnceLock::new();
        let (key, db) = match self.dbs.get_key_value(full) {
            Some((k, d)) => (k.as_str(), d),
            None => (full, EMPTY.get_or_init(Database::default)),
        };
        Ok(DatabaseView {
            db_type: key,
            defs: &self.defs,
            db,
        })
    }

    /// `DefDatabase<T>.GetNamed`: the def named `def_name` in the database of `db_type`.
    #[must_use]
    pub fn get(&self, db_type: &str, def_name: &str) -> Option<&DefRecord> {
        self.database(db_type).ok()?.get(def_name)
    }

    /// Every replacement of a def by a later one, over all databases (sorted by database type, in
    /// the order the replacements happened).
    #[must_use]
    pub fn overrides(&self) -> Vec<Override<'_>> {
        let mut out = Vec::new();
        for (name, db) in &self.dbs {
            for &(old, new) in &db.overridden {
                if let (Some(o), Some(n)) =
                    (self.defs.get(old as usize), self.defs.get(new as usize))
                {
                    out.push(Override {
                        db_type: name,
                        old: o,
                        new: n,
                    });
                }
            }
        }
        out
    }

    /// Every def skipped because its mod already had a def of that name, over all databases.
    #[must_use]
    pub fn duplicates(&self) -> Vec<Duplicate<'_>> {
        let mut out = Vec::new();
        for (name, db) in &self.dbs {
            for &(kept, skipped) in &db.skipped {
                if let (Some(k), Some(s)) = (
                    self.defs.get(kept as usize),
                    self.defs.get(skipped as usize),
                ) {
                    out.push(Duplicate {
                        db_type: name,
                        kept: k,
                        skipped: s,
                    });
                }
            }
        }
        out
    }
}

/// `DefDatabase<T>.AddAllInMods` for one type. `members` are def indices in document order.
fn build_database(
    db_type: &str,
    members: &[u32],
    defs: &[DefRecord],
    mod_order: &[ModIdx],
    diag: &mut DiagSink,
) -> Database {
    let mut by_mod: FxHashMap<ModIdx, Vec<u32>> = FxHashMap::default();
    let mut patched: Vec<u32> = Vec::new();
    for &i in members {
        let Some(d) = defs.get(i as usize) else {
            continue;
        };
        match d.mod_idx() {
            Some(m) => by_mod.entry(m).or_default().push(i),
            None => patched.push(i),
        }
    }
    let mut slots: Vec<Option<u32>> = Vec::new();
    let mut pos: FxHashMap<String, usize> = FxHashMap::default();
    let mut overridden: Vec<(u32, u32)> = Vec::new();
    let mut skipped: Vec<(u32, u32)> = Vec::new();
    let mut add = |i: u32, name: &str, slots: &mut Vec<Option<u32>>| {
        if let Some(&p) = pos.get(name)
            && let Some(old) = slots.get_mut(p).and_then(Option::take)
        {
            overridden.push((old, i));
        }
        slots.push(Some(i));
        pos.insert(name.to_owned(), slots.len().saturating_sub(1));
    };
    let mut remaining: Vec<ModIdx> = by_mod
        .keys()
        .copied()
        .filter(|m| !mod_order.contains(m))
        .collect();
    remaining.sort_unstable();
    for m in mod_order.iter().chain(remaining.iter()) {
        let Some(list) = by_mod.get(m) else { continue };
        let mut seen: FxHashMap<&str, u32> = FxHashMap::default();
        for &i in list {
            let Some(d) = defs.get(i as usize) else {
                continue;
            };
            if let Some(&kept) = seen.get(d.def_name.as_str()) {
                skipped.push((kept, i));
                diag.push(
                    Site::new(d.origin.map(|o| o.mod_idx), d.origin.map(|o| o.file))
                        .diag(
                            diag_codes::DUPLICATE_IN_MOD,
                            Severity::Error,
                            format!(
                                "A mod has multiple {db_type}s named {}. Skipping.",
                                d.def_name
                            ),
                        )
                        .with_arg("defName", d.def_name.clone())
                        .with_arg("type", db_type),
                );
                continue;
            }
            seen.insert(d.def_name.as_str(), i);
            add(i, &d.def_name, &mut slots);
        }
    }
    for &i in &patched {
        if let Some(d) = defs.get(i as usize) {
            add(i, &d.def_name, &mut slots);
        }
    }
    let entries: Vec<u32> = slots.into_iter().flatten().collect();
    let by_name = entries
        .iter()
        .filter_map(|&i| defs.get(i as usize).map(|d| (d.def_name.clone(), i)))
        .collect();
    Database {
        entries,
        by_name,
        overridden,
        skipped,
    }
}

// ---------------------------------------------------------------------------- load

/// Everything [`load`] needs. The files come from the boundary and library layers, already read,
/// parsed and ordered; this crate does no IO.
#[derive(Debug, Clone)]
pub struct LoadInput {
    /// The mods in load order (position 0 loads first).
    pub mods: Vec<ModEntry>,
    /// The defs files in load order: mods in load order, and inside a mod in the game's file order
    /// (highest priority load folder first, ordinal path order inside a folder).
    pub def_files: Vec<DefFile>,
    /// The patch files, in the same order (operations apply mod by mod, file by file).
    pub patch_files: Vec<PatchFile>,
    /// Package ids that count as active although no mod entry exists for them.
    pub extra_active_ids: Vec<String>,
    /// The def type table.
    pub types: Arc<TypeTable>,
    /// Inheritance settings.
    pub inherit: InheritConfig,
    /// Handlers for custom patch classes.
    pub custom_ops: CustomRegistry,
}

impl LoadInput {
    /// An input with no files, default inheritance settings and no custom handlers.
    #[must_use]
    pub fn new(mods: Vec<ModEntry>, types: Arc<TypeTable>) -> Self {
        Self {
            mods,
            def_files: Vec::new(),
            patch_files: Vec::new(),
            extra_active_ids: Vec::new(),
            types,
            inherit: InheritConfig::default(),
            custom_ops: CustomRegistry::new(),
        }
    }
}

/// Microseconds spent per stage. Timings vary from run to run and are not part of the
/// deterministic output.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadTimings {
    /// Parsing the patch files into operations.
    pub parse_patches: u64,
    /// Merging the defs files.
    pub merge: u64,
    /// Applying the patches.
    pub patch: u64,
    /// Resolving inheritance.
    pub inherit: u64,
    /// Creating the defs.
    pub defs: u64,
    /// Building the databases.
    pub databases: u64,
    /// The whole load.
    pub total: u64,
}

/// The result of [`load`].
#[derive(Debug, Clone)]
pub struct LoadOutput {
    /// The defs in merged document order.
    pub defs: Arc<Vec<DefRecord>>,
    /// The per type databases.
    pub databases: DefDatabases,
    /// One event per top-level patch operation.
    pub patch_report: PatchReport,
    /// Every diagnostic of the load, counted per code with bounded deterministic samples.
    pub diagnostics: DiagnosticSummary,
    /// Counters equal to the research prototype's `stats`.
    pub stats: LoadStats,
    /// Time per stage.
    pub timings: LoadTimings,
    /// The mods of the load in load order.
    pub order: ModOrder,
}

impl LoadOutput {
    /// `DefDatabase<T>.GetNamed`.
    #[must_use]
    pub fn get(&self, db_type: &str, def_name: &str) -> Option<&DefRecord> {
        self.databases.get(db_type, def_name)
    }
}

fn micros(t: Instant) -> u64 {
    u64::try_from(t.elapsed().as_micros()).unwrap_or(u64::MAX)
}

/// The def pipeline end to end: merge, patch, inherit, create defs, build databases.
///
/// Content problems never abort the load; they are in [`LoadOutput::diagnostics`]. Patch files are
/// parsed in parallel and trees are resolved in parallel; the output is byte identical for any
/// thread count (only [`LoadOutput::timings`] differ).
#[must_use]
pub fn load(input: LoadInput) -> LoadOutput {
    let started = Instant::now();
    let mut timings = LoadTimings::default();
    let order = ModOrder::new(input.mods);
    let patch_ctx = PatchContext::for_mods(&order, &input.extra_active_ids, input.custom_ops);
    let mut sink = DiagSink::new();

    // patch files -> operations, in parallel, grouped per mod in load order
    let t = Instant::now();
    let parsed: Vec<(ModIdx, Vec<PatchOp>, DiagSink)> = input
        .patch_files
        .par_iter()
        .map(|pf| {
            let mut local = DiagSink::new();
            let ops = match &pf.content {
                FileContent::Parsed { root } => {
                    let env = ParseEnv {
                        registry: &patch_ctx.custom,
                        active: &patch_ctx.active,
                    };
                    parse_patch_file(root, &env, Some(pf.mod_idx), Some(pf.file), &mut local)
                }
                FileContent::Failed { message } => {
                    let site = Site::new(Some(pf.mod_idx), Some(pf.file));
                    local.push(site.diag(
                        diag_codes::XML_PARSE_ERROR,
                        Severity::Warning,
                        format!("Exception reading {} as XML: {message}", pf.rel_path),
                    ));
                    local.push(site.diag(
                        diag_codes::PATCH_FILE_UNREADABLE,
                        Severity::Error,
                        format!(
                            "patch file has no document (the game would throw): {}",
                            pf.rel_path
                        ),
                    ));
                    Vec::new()
                }
            };
            (pf.mod_idx, ops, local)
        })
        .collect();
    let mut per_mod: FxHashMap<ModIdx, Vec<PatchOp>> = FxHashMap::default();
    for (m, ops, local) in parsed {
        sink.merge(local);
        per_mod.entry(m).or_default().extend(ops);
    }
    let mut patches: Vec<(ModIdx, Vec<PatchOp>)> = Vec::new();
    let mut placed: FxHashSet<ModIdx> = FxHashSet::default();
    for e in order.iter() {
        if let Some(ops) = per_mod.remove(&e.idx)
            && placed.insert(e.idx)
        {
            patches.push((e.idx, ops));
        }
    }
    let mut rest: Vec<(ModIdx, Vec<PatchOp>)> = per_mod.into_iter().collect();
    rest.sort_by_key(|(m, _)| *m);
    patches.extend(rest);
    timings.parse_patches = micros(t);

    // merge
    let t = Instant::now();
    let mut doc = merge(&order, &input.def_files);
    sink.merge(doc.take_diagnostics());
    timings.merge = micros(t);

    // patch
    let t = Instant::now();
    let patch_report = apply_patches(&mut doc, &patches, &patch_ctx);
    sink.merge(patch_report.diagnostics.clone());
    timings.patch = micros(t);

    // inherit
    let t = Instant::now();
    let resolved = resolve_inheritance(&doc, &patch_ctx.active, &input.inherit);
    sink.merge(resolved.diagnostics.clone());
    let top_level_nodes = u32::try_from(resolved.nodes.len()).unwrap_or(u32::MAX);
    timings.inherit = micros(t);

    // defs
    let t = Instant::now();
    let built = build_defs_with(resolved, &input.types, &patch_ctx.active, &mut sink);
    let stats = LoadStats {
        files: doc.file_count(),
        patch_ops: u32::try_from(patch_report.events.len()).unwrap_or(u32::MAX),
        top_level_nodes,
        abstract_nodes: built.abstract_nodes,
        may_require_skipped: built.may_require_skipped,
        unknown_type: built.unknown_type,
        defs: u32::try_from(built.defs.len()).unwrap_or(u32::MAX),
    };
    let defs = Arc::new(built.defs);
    timings.defs = micros(t);

    // databases
    let t = Instant::now();
    let databases = DefDatabases::build_shared(Arc::clone(&defs), input.types, &order);
    sink.merge(databases.diagnostics().clone());
    timings.databases = micros(t);
    timings.total = micros(started);

    LoadOutput {
        defs,
        databases,
        patch_report,
        diagnostics: sink.finish(),
        stats,
        timings,
        order,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_table::TypeInfo;
    use rimstudio_core::ids::FileId;
    use rimstudio_core::tree::NodeBuilder;

    fn types() -> Arc<TypeTable> {
        Arc::new(
            TypeTable::new(vec![
                ("Verse.Def".into(), TypeInfo::with_base("Verse.Editable")),
                ("Verse.RS_ThingDef".into(), TypeInfo::with_base("Verse.Def")),
                (
                    "RS_Mod.RS_AmmoDef".into(),
                    TypeInfo::with_base("Verse.RS_ThingDef"),
                ),
            ])
            .unwrap(),
        )
    }

    fn defs_file(m: u32, f: u32, nodes: Vec<NodeBuilder>) -> DefFile {
        DefFile {
            mod_idx: ModIdx(m),
            file: FileId(f),
            rel_path: format!("Defs/f{f}.xml"),
            content: FileContent::parsed(NodeBuilder::new("Defs").children(nodes).build()),
        }
    }

    fn thing(tag: &str, name: &str) -> NodeBuilder {
        NodeBuilder::new(tag).text_elem("defName", name)
    }

    fn input(files: Vec<DefFile>) -> LoadInput {
        let mut i = LoadInput::new(
            vec![
                ModEntry::new(ModIdx(0), "ludeon.rimworld", "Core"),
                ModEntry::new(ModIdx(1), "rs.a", "RS A"),
            ],
            types(),
        );
        i.def_files = files;
        i
    }

    #[test]
    fn later_mods_override_and_the_winner_moves_to_the_end() {
        let out = load(input(vec![
            defs_file(
                0,
                0,
                vec![thing("RS_ThingDef", "A"), thing("RS_ThingDef", "B")],
            ),
            defs_file(1, 1, vec![thing("RS_ThingDef", "A")]),
        ]));
        let db = out.databases.database("RS_ThingDef").unwrap();
        assert_eq!(db.names(), vec!["B", "A"]);
        assert_eq!(out.stats.defs, 3);
        assert_eq!(out.stats.files, 2);
        let ov = out.databases.overrides();
        assert_eq!(ov.len(), 1);
        assert!(!ov[0].type_changed());
        assert_eq!(
            out.get("RS_ThingDef", "A").and_then(|d| d.mod_idx()),
            Some(ModIdx(1))
        );
    }

    #[test]
    fn subclass_defs_live_in_the_ancestor_database_and_type_changes_are_reported() {
        let out = load(input(vec![
            defs_file(0, 0, vec![thing("RS_ThingDef", "X")]),
            defs_file(1, 1, vec![thing("RS_Mod.RS_AmmoDef", "X")]),
        ]));
        assert_eq!(
            out.databases.database("RS_ThingDef").unwrap().names(),
            vec!["X"]
        );
        assert_eq!(
            out.databases.database("RS_Mod.RS_AmmoDef").unwrap().names(),
            vec!["X"]
        );
        let ov = out.databases.overrides();
        assert_eq!(ov.len(), 1);
        assert!(ov[0].type_changed());
        assert_eq!(ov[0].db_type, "Verse.RS_ThingDef");
    }

    #[test]
    fn duplicates_inside_one_mod_are_skipped_and_reported() {
        let out = load(input(vec![defs_file(
            0,
            0,
            vec![thing("RS_ThingDef", "X"), thing("RS_ThingDef", "X")],
        )]));
        assert_eq!(out.databases.database("RS_ThingDef").unwrap().len(), 1);
        assert_eq!(out.diagnostics.count(&diag_codes::DUPLICATE_IN_MOD), 1);
        assert_eq!(out.databases.duplicates().len(), 1);
    }

    #[test]
    fn unknown_types_and_abstract_nodes_are_counted_not_created() {
        let out = load(input(vec![defs_file(
            0,
            0,
            vec![
                thing("RS_Bogus", "A"),
                NodeBuilder::new("RS_ThingDef")
                    .attr("Abstract", "TRUE")
                    .text_elem("defName", "B"),
                thing("RS_ThingDef", "C"),
            ],
        )]));
        assert_eq!(out.stats.unknown_type, 1);
        assert_eq!(out.stats.abstract_nodes, 1);
        assert_eq!(out.stats.defs, 1);
        assert_eq!(out.stats.top_level_nodes, 3);
        assert_eq!(out.diagnostics.count(&diag_codes::UNKNOWN_TYPE), 1);
    }

    #[test]
    fn database_errors_are_typed() {
        let out = load(input(vec![]));
        assert!(matches!(
            out.databases.database("Nope"),
            Err(DefsError::UnknownType { .. })
        ));
        assert!(matches!(
            out.databases.database("Verse.Def"),
            Err(DefsError::RootType { .. })
        ));
        assert!(out.databases.database("RS_ThingDef").unwrap().is_empty());
    }

    #[test]
    fn a_def_without_a_name_gets_a_generated_one_and_a_warning() {
        let out = load(input(vec![defs_file(
            0,
            0,
            vec![NodeBuilder::new("RS_ThingDef").text_elem("label", "x")],
        )]));
        assert_eq!(out.defs[0].def_name, "UnnamedDef0");
        assert_eq!(out.diagnostics.count(&diag_codes::UNNAMED_DEF), 1);
    }

    #[test]
    fn patch_created_defs_enter_the_database_last() {
        let mut i = input(vec![defs_file(
            0,
            0,
            vec![thing("RS_ThingDef", "R"), thing("RS_ThingDef", "Z")],
        )]);
        let patch = NodeBuilder::new("Patch")
            .child(
                NodeBuilder::new("Operation")
                    .attr("Class", "PatchOperationReplace")
                    .text_elem("xpath", "/Defs/RS_ThingDef[defName=\"R\"]")
                    .child(
                        NodeBuilder::new("value")
                            .child(thing("RS_ThingDef", "R").text_elem("label", "replaced")),
                    ),
            )
            .build();
        i.patch_files = vec![PatchFile {
            mod_idx: ModIdx(0),
            file: FileId(9),
            rel_path: "Patches/p.xml".into(),
            content: FileContent::parsed(patch),
        }];
        let out = load(i);
        assert_eq!(out.patch_report.results(), vec![true]);
        let db = out.databases.database("RS_ThingDef").unwrap();
        assert_eq!(db.names(), vec!["Z", "R"]);
        assert_eq!(out.get("RS_ThingDef", "R").map(|d| d.origin), Some(None));
    }
}
