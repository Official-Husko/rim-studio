//! The context of the designer: a narrow handle on everything a designer call needs.
//!
//! [`Ctx`] holds the workspace session (absent when no game install is known: the designer then still
//! computes exact readouts and keeps drafts), the data roots, a clock and the document store handles. It
//! also owns the lazily built [`Engine`]: the reference weapons, the two pools, the baseline models and the
//! Combat Extended model of the current def snapshot. The engine is rebuilt when the session publishes a new
//! snapshot, never copied per call, so a preview is a lookup plus a little arithmetic.
//!
//! The context is deliberately small so that the second half of the toolkit (write plans, apply, convert)
//! can extend it without touching the first: it exposes the session, the roots, the clock and the engine
//! through accessors, and the stores of this half stay crate private.

use std::sync::{Arc, Mutex, OnceLock};

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ids::ModIdx;
use rimstudio_core::ports::Clock;
use rimstudio_defs::DefDatabases;
use rimstudio_design::baseline::{BaselineConfig, Identity, Model, StatRule};
use rimstudio_design::ce::reader::{CeModel, CeReadOptions, read_conversions_with};
use rimstudio_design::classes::{ItemKind as PoolKind, Pool};
use rimstudio_design::reader::{ReaderOptions, ReferencePools, build_pools};
use rimstudio_design::validation::{DefLookup, RefKind};
use rimstudio_io::collection::{Collection, CollectionOptions};
use rimstudio_io::roots::{DataRoots, RootKind};
use rimstudio_workspace::defindex::{DefIndex, Page, Query};
use rimstudio_workspace::refset::PackKind;
use rimstudio_workspace::session::{OpenInput, WorkspaceSession};
use rimstudio_workspace::snapshot::Snapshot;

use super::calibrate::{CalibrationRecord, CalibrationState, calibration_key, validate_options};
use super::drafts::DraftRecord;
use crate::error::{ToolkitError, ToolkitResult};
use crate::shared::env::ProjectEnv;

/// Name of the draft collection in the data root (D-083).
pub const DRAFTS_COLLECTION: &str = "designer-drafts";
/// Name of the calibration collection in the cache root.
pub const CALIBRATION_COLLECTION: &str = "designer-calibration";

/// The designer's view of one def snapshot: reference weapons, pools, models and the Combat Extended model.
///
/// Everything is computed on first use and kept for the life of the snapshot. Reading it never touches the
/// disk. A weapon of Combat Extended never enters the vanilla pools (the reader leaves defs with a custom
/// verb or tool class out), so the vanilla numbers stay vanilla even when Combat Extended is loaded as a
/// reference mod.
#[derive(Debug)]
pub struct Engine {
    snapshot: Arc<Snapshot>,
    index: Arc<DefIndex>,
    reader: ReaderOptions,
    pools: OnceLock<Result<Arc<ReferencePools>, String>>,
    models: [OnceLock<Option<Arc<Model>>>; 2],
    keys: [OnceLock<String>; 2],
    ce_snapshot: Option<Arc<Snapshot>>,
    ce: OnceLock<Arc<CeModel>>,
}

impl Engine {
    fn new(
        session: &WorkspaceSession,
        ce_session: Option<&WorkspaceSession>,
        reader: ReaderOptions,
    ) -> Self {
        Self {
            ce_snapshot: ce_session.map(WorkspaceSession::current),
            snapshot: session.current(),
            index: session.index(),
            reader,
            pools: OnceLock::new(),
            models: [OnceLock::new(), OnceLock::new()],
            keys: [OnceLock::new(), OnceLock::new()],
            ce: OnceLock::new(),
        }
    }

    /// The def snapshot the engine reads.
    #[must_use]
    pub fn snapshot(&self) -> &Arc<Snapshot> {
        &self.snapshot
    }

    /// The resolved def databases of the snapshot.
    #[must_use]
    pub fn databases(&self) -> &Arc<DefDatabases> {
        &self.snapshot.databases
    }

    /// The def index (abstract nodes included) of the session.
    #[must_use]
    pub fn index(&self) -> &Arc<DefIndex> {
        &self.index
    }

    /// The options the reference reader runs with.
    #[must_use]
    pub fn reader_options(&self) -> &ReaderOptions {
        &self.reader
    }

    /// The reference weapons and the two pools.
    ///
    /// # Errors
    ///
    /// [`ToolkitError::ReferenceUnavailable`] when the loaded defs have no weapon database at all.
    pub fn pools(&self) -> ToolkitResult<&ReferencePools> {
        let built = self.pools.get_or_init(|| {
            build_pools(&self.snapshot.databases, &self.reader)
                .map(Arc::new)
                .map_err(|e| e.to_string())
        });
        match built {
            Ok(p) => Ok(p.as_ref()),
            Err(reason) => Err(ToolkitError::ReferenceUnavailable {
                reason: reason.clone(),
            }),
        }
    }

    /// The reference pool of one kind.
    ///
    /// # Errors
    ///
    /// See [`Engine::pools`].
    pub fn pool(&self, kind: PoolKind) -> ToolkitResult<&Pool> {
        let pools = self.pools()?;
        Ok(match kind {
            PoolKind::Melee => &pools.melee,
            _ => &pools.ranged,
        })
    }

    /// The baseline model fitted on the pool of `kind`, or `None` when the pool is empty.
    ///
    /// # Errors
    ///
    /// See [`Engine::pools`].
    pub fn model(&self, kind: PoolKind) -> ToolkitResult<Option<Arc<Model>>> {
        let slot = match kind {
            PoolKind::Melee => &self.models[1],
            _ => &self.models[0],
        };
        let pool = self.pool(kind)?;
        let model = slot.get_or_init(|| {
            if pool.items.is_empty() {
                None
            } else {
                Some(Arc::new(Model::fit(pool.clone(), baseline_config(kind))))
            }
        });
        Ok(model.clone())
    }

    /// The calibration cache key of the pool of `kind` (see [`calibration_key`]), computed once.
    ///
    /// # Errors
    ///
    /// See [`Engine::pools`].
    pub fn calibration_key(&self, kind: PoolKind) -> ToolkitResult<&str> {
        let slot = match kind {
            PoolKind::Melee => &self.keys[1],
            _ => &self.keys[0],
        };
        let pool = self.pool(kind)?;
        Ok(slot.get_or_init(|| {
            calibration_key(pool, &baseline_config(kind), &validate_options(None, 1))
        }))
    }

    /// The Combat Extended model. [`CeModel::is_present`] is false when no Combat Extended data is loaded;
    /// nothing about Combat Extended is then available anywhere in the designer.
    ///
    /// With a Combat Extended session ([`Ctx::with_ce_session`]) the model is read from that session and
    /// paired with the vanilla twins of the main session; without one it is read from the main session
    /// itself (a session that has Combat Extended among its reference mods).
    #[must_use]
    pub fn ce(&self) -> &CeModel {
        self.ce
            .get_or_init(|| {
                let model = match &self.ce_snapshot {
                    Some(ce) => {
                        let opts = CeReadOptions {
                            order: Some(&ce.order),
                            vanilla: Some(self.snapshot.databases.as_ref()),
                            ..CeReadOptions::default()
                        };
                        read_conversions_with(&ce.databases, &opts)
                    }
                    None => {
                        let opts = CeReadOptions {
                            order: Some(&self.snapshot.order),
                            ..CeReadOptions::default()
                        };
                        read_conversions_with(&self.snapshot.databases, &opts)
                    }
                };
                Arc::new(model)
            })
            .as_ref()
    }

    /// True when Combat Extended data is part of the reference set, so the optional patch can be offered.
    #[must_use]
    pub fn ce_available(&self) -> bool {
        self.ce().is_present()
    }

    /// The package id of the mod that defines items of the pack handle `idx`, `None` for the base game and
    /// the official expansions.
    #[must_use]
    pub fn mod_id_of(&self, idx: ModIdx) -> Option<&str> {
        let pack = self.snapshot.pack(usize::try_from(idx.0).ok()?)?;
        match pack.kind {
            PackKind::Mod | PackKind::Project => Some(pack.package_id.as_str()),
            PackKind::Core | PackKind::Dlc => None,
        }
    }

    /// A [`DefLookup`] over the snapshot for the reference checks of the validator.
    #[must_use]
    pub fn lookup(&self) -> SessionLookup<'_> {
        SessionLookup { engine: self }
    }
}

/// The baseline configuration of a pool kind: the stats that are asked instead of estimated and the
/// structural identities of the math specification.
///
/// Work to make is not predictable (about 50 percent error) and is always an input. Melee swing damage is
/// the strength index times the swing cooldown.
#[must_use]
pub fn baseline_config(kind: PoolKind) -> BaselineConfig {
    let mut config = BaselineConfig::default();
    for stat in ["work", "market_value"] {
        config.rules.push(StatRule {
            name: stat.to_owned(),
            discrete: None,
            integer: None,
            ask: true,
        });
    }
    if kind == PoolKind::Melee {
        config.identities.push(Identity::StrengthTimes {
            stat: "swing_damage".to_owned(),
            other: "cooldown".to_owned(),
        });
    }
    config
}

/// Answers the validator's reference questions from the def snapshot.
///
/// A kind whose database does not exist in the load answers `None` (unknown), which produces no diagnostic,
/// so a partial reference set never invents errors.
#[derive(Debug, Clone, Copy)]
pub struct SessionLookup<'a> {
    engine: &'a Engine,
}

fn type_of(kind: RefKind) -> &'static str {
    match kind {
        RefKind::Thing => "ThingDef",
        RefKind::ResearchProject => "ResearchProjectDef",
        RefKind::BodyPartGroup => "BodyPartGroupDef",
        RefKind::ToolCapacity => "ToolCapacityDef",
        RefKind::DamageDef => "DamageDef",
        RefKind::StuffCategory => "StuffCategoryDef",
        RefKind::WeaponClass => "WeaponClassDef",
        RefKind::SoundDef => "SoundDef",
        RefKind::SkillDef => "SkillDef",
    }
}

impl SessionLookup<'_> {
    /// True when the index holds a node of this type that is called `name`: a concrete def by its
    /// `defName` or an abstract base by its `Name` attribute (abstract bases have no `defName`).
    fn abstract_base_exists(&self, def_type: &str, name: &str) -> bool {
        let index = self.engine.index();
        if !index.find(Some(def_type), name).is_empty() {
            return true;
        }
        let query = Query::text(name).of_type(def_type);
        index
            .search(&query, Page::new(0, 200))
            .hits
            .iter()
            .any(|h| h.name.as_deref() == Some(name) || h.def_name == name)
    }
}

impl DefLookup for SessionLookup<'_> {
    fn contains(&self, kind: RefKind, name: &str) -> Option<bool> {
        let db_type = type_of(kind);
        let dbs = self.engine.databases();
        let view = dbs.database(db_type).ok()?;
        if view.is_empty() {
            return None;
        }
        if view.get(name).is_some() {
            return Some(true);
        }
        // Parent bases are abstract nodes; the databases hold only resolved defs, the index holds both.
        if kind == RefKind::Thing && self.abstract_base_exists(db_type, name) {
            return Some(true);
        }
        Some(false)
    }
}

/// What the cache said about each calibration key, kept in memory after the first lookup.
type CalibrationMemory = Mutex<Vec<(String, CalibrationState)>>;

/// The context of a designer call.
///
/// Build it once per app session with [`Ctx::new`] (or [`Ctx::offline`] without a game install) and share
/// it by reference; when the workspace session is replaced, [`Ctx::with_session`] gives a context that
/// keeps the stores and drops the caches of the old snapshot.
pub struct Ctx {
    session: Option<Arc<WorkspaceSession>>,
    ce_session: Option<Arc<WorkspaceSession>>,
    roots: DataRoots,
    clock: Arc<dyn Clock>,
    reader: ReaderOptions,
    drafts: Collection<DraftRecord>,
    calibrations: Collection<CalibrationRecord>,
    engine: Mutex<Option<Arc<Engine>>>,
    calibration_memory: CalibrationMemory,
    env: ProjectEnv,
    project_sessions: Mutex<Option<ProjectSession>>,
}

/// The cached session over the reference set plus the open project, see [`Ctx::project_session`].
struct ProjectSession {
    root: Utf8PathBuf,
    base_revision: u64,
    base_ce: bool,
    session: Arc<WorkspaceSession>,
}

impl std::fmt::Debug for Ctx {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Ctx")
            .field("has_session", &self.session.is_some())
            .field("roots", &self.roots)
            .finish_non_exhaustive()
    }
}

impl Ctx {
    /// A context over a workspace session, with the stores opened in `roots` (drafts in the data root,
    /// calibration results in the cache root).
    ///
    /// # Errors
    ///
    /// [`ToolkitError::Store`] when a store folder cannot be created.
    pub fn new(
        session: Arc<WorkspaceSession>,
        roots: &DataRoots,
        clock: Arc<dyn Clock>,
    ) -> ToolkitResult<Self> {
        let mut ctx = Self::offline(roots, clock)?;
        ctx.session = Some(session);
        Ok(ctx)
    }

    /// A context without a game install: drafts and the pure formulas work, references, suggestions, the
    /// fit meter, the quiz and calibration report [`ToolkitError::ReferenceUnavailable`] or leave their
    /// parts empty.
    ///
    /// # Errors
    ///
    /// [`ToolkitError::Store`] when a store folder cannot be created.
    pub fn offline(roots: &DataRoots, clock: Arc<dyn Clock>) -> ToolkitResult<Self> {
        let drafts = Collection::<DraftRecord>::open(
            roots,
            RootKind::Data,
            DRAFTS_COLLECTION,
            clock.clone(),
        )?
        .with_summarizer(DraftRecord::summary);
        let calibrations = Collection::<CalibrationRecord>::open(
            roots,
            RootKind::Cache,
            CALIBRATION_COLLECTION,
            clock.clone(),
        )?
        .with_options(CollectionOptions::cache())
        .with_summarizer(CalibrationRecord::summary);
        let env = ProjectEnv::new(roots, clock.clone())?;
        Ok(Self {
            session: None,
            ce_session: None,
            roots: roots.clone(),
            clock,
            reader: ReaderOptions::default(),
            drafts,
            calibrations,
            engine: Mutex::new(None),
            calibration_memory: Mutex::new(Vec::new()),
            env,
            project_sessions: Mutex::new(None),
        })
    }

    /// A context over another session that shares this context's stores and options.
    #[must_use]
    pub fn with_session(&self, session: Arc<WorkspaceSession>) -> Self {
        Self {
            session: Some(session),
            ce_session: self.ce_session.clone(),
            roots: self.roots.clone(),
            clock: self.clock.clone(),
            reader: self.reader.clone(),
            drafts: self.drafts.clone(),
            calibrations: self.calibrations.clone(),
            engine: Mutex::new(None),
            calibration_memory: Mutex::new(Vec::new()),
            env: self.env.clone(),
            project_sessions: Mutex::new(None),
        }
    }

    /// The same context with a second session that holds Combat Extended as a reference mod (with the
    /// gun conversion operation registered by the caller). The vanilla pools, the baseline and the
    /// calibration keep coming from the main session, which has no Combat Extended; the optional patch
    /// reads the Combat Extended model from this one and pairs its guns with their vanilla twins.
    #[must_use]
    pub fn with_ce_session(mut self, session: Arc<WorkspaceSession>) -> Self {
        self.ce_session = Some(session);
        self.engine = Mutex::new(None);
        self
    }

    /// The Combat Extended session, when one was given.
    #[must_use]
    pub fn ce_session(&self) -> Option<&Arc<WorkspaceSession>> {
        self.ce_session.as_ref()
    }

    /// The same context with other reader options (a reference material for stuffed weapons, other
    /// exclusions or role rules). The engine is rebuilt on next use.
    #[must_use]
    pub fn with_reader_options(mut self, reader: ReaderOptions) -> Self {
        self.reader = reader;
        self.engine = Mutex::new(None);
        self.calibration_memory = Mutex::new(Vec::new());
        self
    }

    /// The workspace session, when a game install is known.
    #[must_use]
    pub fn session(&self) -> Option<&Arc<WorkspaceSession>> {
        self.session.as_ref()
    }

    /// The data roots the stores live in.
    #[must_use]
    pub fn roots(&self) -> &DataRoots {
        &self.roots
    }

    /// The clock of the context.
    #[must_use]
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    /// Milliseconds since the Unix epoch by the context's clock.
    #[must_use]
    pub fn now_ms(&self) -> u64 {
        self.clock.now_unix_ms()
    }

    /// The options of the reference reader.
    #[must_use]
    pub fn reader_options(&self) -> &ReaderOptions {
        &self.reader
    }

    /// The engine of the current snapshot, `None` without a session.
    #[must_use]
    pub fn engine(&self) -> Option<Arc<Engine>> {
        let session = self.session.as_ref()?;
        let mut slot = self.engine.lock().unwrap_or_else(|p| p.into_inner());
        let current = session.snapshot();
        let ce_current = self.ce_session.as_ref().map(|s| s.snapshot());
        if let Some(engine) = slot.as_ref()
            && Arc::ptr_eq(engine.databases(), &current)
            && match (&engine.ce_snapshot, &ce_current) {
                (Some(a), Some(b)) => Arc::ptr_eq(&a.databases, b),
                (None, None) => true,
                _ => false,
            }
        {
            return Some(engine.clone());
        }
        let engine = Arc::new(Engine::new(
            session,
            self.ce_session.as_deref(),
            self.reader.clone(),
        ));
        *slot = Some(engine.clone());
        Some(engine)
    }

    /// The engine of the current snapshot.
    ///
    /// # Errors
    ///
    /// [`ToolkitError::ReferenceUnavailable`] without a session.
    pub fn require_engine(&self) -> ToolkitResult<Arc<Engine>> {
        self.engine()
            .ok_or_else(|| ToolkitError::ReferenceUnavailable {
                reason: "no game install is loaded".to_owned(),
            })
    }

    /// The project environment: the project store, the protected folders and the writer factory.
    #[must_use]
    pub fn env(&self) -> &ProjectEnv {
        &self.env
    }

    /// The same context with a different project environment (protected folders, a write fence).
    #[must_use]
    pub fn with_env(mut self, env: ProjectEnv) -> Self {
        self.env = env;
        self
    }

    /// The folders no write may touch: the protected folders of the environment, the game install of the
    /// reference set (the folder that holds `Data`) and the roots of every reference pack of the main and
    /// the Combat Extended session. A project that lives inside one of them cannot be written.
    #[must_use]
    pub fn protected_paths(&self) -> Vec<Utf8PathBuf> {
        let mut out: Vec<Utf8PathBuf> = self.env.protected().to_vec();
        for session in self.session.iter().chain(self.ce_session.iter()) {
            for pack in &session.reference().packs {
                match pack.kind {
                    PackKind::Core | PackKind::Dlc => {
                        let install = pack
                            .root
                            .parent()
                            .filter(|data| data.file_name() == Some("Data"))
                            .and_then(Utf8Path::parent);
                        out.push(install.map_or_else(|| pack.root.clone(), Utf8Path::to_path_buf));
                    }
                    PackKind::Mod => out.push(pack.root.clone()),
                    PackKind::Project => {}
                }
            }
        }
        out.sort();
        out.dedup();
        out
    }

    /// A guarded writer for a registered project, with every protected folder of this context applied.
    ///
    /// # Errors
    ///
    /// [`ToolkitError::PathRefused`] when the project lies inside a protected folder.
    pub fn writer_for(
        &self,
        project_id: &str,
        root: &Utf8Path,
    ) -> ToolkitResult<crate::shared::writer::GuardedWriter> {
        let protected: Vec<Utf8PathBuf> = self
            .protected_paths()
            .into_iter()
            .filter(|p| p.as_path() != root)
            .collect();
        self.env.writer(root, project_id, &protected)
    }

    /// A workspace session over the reference set plus the project folder as its last pack, so the defs of
    /// the project are seen next to the reference defs (the convert flow reads weapons and conversions
    /// from it). The reference set is the Combat Extended session's when there is one (the project's own
    /// Combat Extended patches only apply when Combat Extended is active), else the main session's.
    ///
    /// The session is cached per project folder and rebuilt when a file of the project changed on disk or
    /// the reference snapshot was replaced.
    ///
    /// # Errors
    ///
    /// [`ToolkitError::ReferenceUnavailable`] without a session, [`ToolkitError::Workspace`] when the
    /// project folder is not a mod.
    pub fn project_session(&self, root: &Utf8Path) -> ToolkitResult<Arc<WorkspaceSession>> {
        let base = self
            .ce_session
            .as_ref()
            .or(self.session.as_ref())
            .ok_or_else(|| ToolkitError::ReferenceUnavailable {
                reason: "no game install is loaded".to_owned(),
            })?;
        let mut slot = self
            .project_sessions
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if let Some(cached) = slot.as_ref()
            && cached.root == root
            && cached.base_revision == base.revision()
            && cached.base_ce == self.ce_session.is_some()
            && !cached.session.is_stale()
        {
            return Ok(cached.session.clone());
        }
        let reference = base
            .reference()
            .with_project(root, &rimstudio_workspace::listing::DiskListing)?;
        let mut input = OpenInput::new(reference)
            .with_type_table(base.type_table())
            .with_parse_cache(base.parse_cache());
        input.custom_ops = rimstudio_design::ce::reader::custom_registry(
            &rimstudio_design::ce::reader::CeClassNames::default(),
            std::collections::BTreeMap::new(),
        );
        let session = Arc::new(WorkspaceSession::open_simple(input)?);
        *slot = Some(ProjectSession {
            root: root.to_path_buf(),
            base_revision: base.revision(),
            base_ce: self.ce_session.is_some(),
            session: session.clone(),
        });
        Ok(session)
    }

    pub(crate) fn drafts(&self) -> &Collection<DraftRecord> {
        &self.drafts
    }

    pub(crate) fn calibrations(&self) -> &Collection<CalibrationRecord> {
        &self.calibrations
    }

    pub(crate) fn remembered_calibration(&self, key: &str) -> Option<CalibrationState> {
        let memory = self
            .calibration_memory
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        memory
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, s)| s.clone())
    }

    pub(crate) fn remember_calibration(&self, key: &str, state: CalibrationState) {
        let mut memory = self
            .calibration_memory
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        memory.retain(|(k, _)| k != key);
        memory.push((key.to_owned(), state));
        // Two kinds and a few reference sets are enough; older results are on disk.
        if memory.len() > 8 {
            memory.remove(0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_context_can_be_shared_between_threads() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<Ctx>();
        assert_send_sync::<Engine>();
    }

    #[test]
    fn work_and_price_are_asked_for_both_kinds() {
        for kind in [PoolKind::Ranged, PoolKind::Melee] {
            let config = baseline_config(kind);
            for stat in ["work", "market_value"] {
                assert!(config.rule(stat).is_some_and(|r| r.ask), "{stat}");
            }
        }
    }

    #[test]
    fn only_melee_has_the_strength_times_cooldown_identity() {
        assert!(baseline_config(PoolKind::Ranged).identities.is_empty());
        assert_eq!(baseline_config(PoolKind::Melee).identities.len(), 1);
    }

    #[test]
    fn every_reference_kind_maps_to_a_def_type() {
        for kind in [
            RefKind::Thing,
            RefKind::ResearchProject,
            RefKind::BodyPartGroup,
            RefKind::ToolCapacity,
            RefKind::DamageDef,
            RefKind::StuffCategory,
            RefKind::WeaponClass,
            RefKind::SoundDef,
            RefKind::SkillDef,
        ] {
            assert!(type_of(kind).ends_with("Def"));
        }
    }
}
