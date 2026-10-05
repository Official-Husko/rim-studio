//! The workspace hub: builds and owns the toolkit's sessions.
//!
//! The toolkit works on a workspace session (the parsed def snapshot of a reference set). The hub
//! builds it lazily on first use from what the manager knows: the selected game install, its
//! expansions and, when the user has Combat Extended among the scanned mods, a second session that
//! holds it as a reference mod (never used for output unless the user asks for the optional patch).
//! The sessions are cached under a key made of the install, its version and the Combat Extended folder,
//! so a changed override or a newly added mod folder rebuilds them and nothing else does.
//!
//! Without a game install the hub still gives a working offline toolkit context: drafts, projects and
//! the pure formulas work, the reference dependent commands answer `designer.reference-unavailable`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::jobs::{CancelToken, NoopProgress};
use rimstudio_core::paths::CE_PACKAGE_ID;
use rimstudio_core::settings::WorkspaceSettings;
use rimstudio_core::version::GameVersion;
use rimstudio_design::ce::reader::{CeClassNames, custom_registry};
use rimstudio_io::fence::{FenceConfig, GameWriteFence, RealFenceFs};
use rimstudio_io::roots::RootKind;
use rimstudio_io::store::Store;
use rimstudio_ipc_types::error::{ApiError, codes};
use rimstudio_manager::ctx::WORKSPACE_FILE;
use rimstudio_manager::detect::{self, DetectGetReportRequest, DetectRunRequest};
use rimstudio_manager::scan::LibraryScanRequest;
use rimstudio_steam::report::Install;
use rimstudio_toolkit::designer::Ctx as DesignerCtx;
use rimstudio_toolkit::error::ToolkitError;
use rimstudio_toolkit::shared::env::ProjectEnv;
use rimstudio_workspace::refset::{DesignerReferenceOptions, ReferenceSet};
use rimstudio_workspace::session::{OpenInput, WorkspaceSession};
use rimstudio_workspace::typetable::CacheConfig;

use crate::context::AppContext;

/// Which def session a request means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionSelector {
    /// The game, its expansions and nothing else (the default).
    Reference,
    /// The reference set with Combat Extended added, when it was found.
    CombatExtended,
    /// The reference set with an open project as the last pack.
    Project(String),
}

impl SessionSelector {
    /// Parses the `sessionId` of a def request: empty or `reference`, `ce`, or `project:<id>`.
    ///
    /// # Errors
    /// `defs.session-not-found` for anything else.
    pub fn parse(text: &str) -> Result<Self, ApiError> {
        let text = text.trim();
        match text {
            "" | "reference" => Ok(Self::Reference),
            "ce" => Ok(Self::CombatExtended),
            _ => match text.strip_prefix("project:") {
                Some(id) if !id.is_empty() => Ok(Self::Project(id.to_owned())),
                _ => Err(ApiError::new(
                    codes::DEFS_SESSION_NOT_FOUND,
                    format!("unknown def session {text:?}"),
                )
                .detail("sessionId", text)),
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HubKey {
    install: Option<Utf8PathBuf>,
    game_version: Option<String>,
    ce_path: Option<Utf8PathBuf>,
}

struct Built {
    key: HubKey,
    generation: u64,
    ctx: Arc<DesignerCtx>,
}

/// What the manager says about the machine, as far as the hub needs it.
struct Plan {
    install: Option<Install>,
    workspace: WorkspaceSettings,
    ce_path: Option<Utf8PathBuf>,
    user_dir: Option<Utf8PathBuf>,
}

/// Builds the toolkit sessions and tracks the projects opened in this run.
pub struct WorkspaceHub {
    env: ProjectEnv,
    state: Mutex<Option<Built>>,
    opened: Mutex<BTreeSet<String>>,
    generation: AtomicU64,
}

impl std::fmt::Debug for WorkspaceHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceHub").finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

impl WorkspaceHub {
    /// A hub over a project environment (the project store and the protected folders).
    #[must_use]
    pub fn new(env: ProjectEnv) -> Self {
        Self {
            env,
            state: Mutex::new(None),
            opened: Mutex::new(BTreeSet::new()),
            generation: AtomicU64::new(1),
        }
    }

    /// Tells the hub that something it keys on may have changed (an override, a source, a scan).
    /// The next call recomputes the key and rebuilds the sessions only when the key differs.
    pub fn bump(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// The project environment shared by every context the hub builds.
    #[must_use]
    pub fn env(&self) -> &ProjectEnv {
        &self.env
    }

    /// Remembers that a project was opened in this run (the `open-project` capability).
    pub fn note_opened(&self, project_id: &str) {
        lock(&self.opened).insert(project_id.to_owned());
    }

    /// Forgets a project.
    pub fn note_closed(&self, project_id: &str) {
        lock(&self.opened).remove(project_id);
    }

    /// How many projects are open in this run.
    #[must_use]
    pub fn opened_count(&self) -> usize {
        lock(&self.opened).len()
    }

    /// Drops the cached sessions; the next call rebuilds them.
    pub fn invalidate(&self) {
        *lock(&self.state) = None;
    }

    /// True when the sessions are built (a call that needs them is then cheap).
    #[must_use]
    pub fn is_built(&self) -> bool {
        lock(&self.state).is_some()
    }

    fn plan(app: &AppContext) -> Result<Plan, ApiError> {
        let workspace = app.workspace_settings();
        let (install, user_dir) = app
            .with_manager(|m| {
                let cached = detect::get_report(m, DetectGetReportRequest::default())?;
                let report = match cached.report {
                    Some(r) => r,
                    None => detect::run(m, DetectRunRequest::default())?.report,
                };
                let install = detect::select_install(&report, &workspace).cloned();
                let user_dir = report.selected.user_dir.clone();
                Ok::<_, rimstudio_manager::ManagerError>((install, user_dir))
            })
            .map_err(|e| app.manager_error(&e))?;
        let mut ce_path = None;
        if install.is_some() {
            let snapshot = match app.library.current() {
                Some(s) => Some(s),
                None => {
                    crate::api::library::run_scan(
                        app,
                        LibraryScanRequest {
                            level: rimstudio_library::scan::ScanLevel::Metadata,
                            ..LibraryScanRequest::default()
                        },
                        &NoopProgress,
                        &CancelToken::new(),
                    )
                    .ok();
                    app.library.current()
                }
            };
            ce_path = snapshot.and_then(|s| {
                s.index
                    .core()
                    .first_by_package_id(CE_PACKAGE_ID)
                    .map(|(_, meta)| meta.path.clone())
            });
        }
        Ok(Plan {
            install,
            workspace,
            ce_path,
            user_dir,
        })
    }

    /// The toolkit context. Builds the sessions on first use (seconds on a real install) and keeps them
    /// until the install, its version or the Combat Extended folder changes.
    ///
    /// # Errors
    /// The mapped manager, toolkit or workspace error.
    pub fn designer_ctx(&self, app: &AppContext) -> Result<Arc<DesignerCtx>, ApiError> {
        let mut state = lock(&self.state);
        let generation = self.generation.load(Ordering::Acquire);
        if let Some(built) = state.as_ref()
            && built.generation == generation
        {
            return Ok(Arc::clone(&built.ctx));
        }
        let plan = Self::plan(app)?;
        let key = HubKey {
            install: plan.install.as_ref().map(|i| i.game_root.clone()),
            game_version: plan
                .install
                .as_ref()
                .and_then(|i| i.version.as_ref())
                .map(|v| v.raw.clone()),
            ce_path: plan.ce_path.clone(),
        };
        if let Some(built) = state.as_mut()
            && built.key == key
        {
            built.generation = generation;
            return Ok(Arc::clone(&built.ctx));
        }
        let ctx = Arc::new(self.build(app, &plan)?);
        *state = Some(Built {
            key,
            generation,
            ctx: Arc::clone(&ctx),
        });
        Ok(ctx)
    }

    /// The folders no write may touch: with built sessions the toolkit's own list (game install,
    /// reference mods and the rest), else the list the plan of the manager gives.
    ///
    /// # Errors
    /// The mapped manager error.
    pub fn protected_paths(&self, app: &AppContext) -> Result<Vec<Utf8PathBuf>, ApiError> {
        if let Some(built) = lock(&self.state).as_ref() {
            return Ok(built.ctx.protected_paths());
        }
        let plan = Self::plan(app)?;
        Ok(Self::protected(&plan))
    }

    fn protected(plan: &Plan) -> Vec<Utf8PathBuf> {
        let mut out = Vec::new();
        if let Some(install) = &plan.install {
            out.push(install.game_root.clone());
            out.push(install.path.clone());
            for w in &install.workshop {
                out.push(w.content_dir.clone());
            }
        }
        out.extend(plan.workspace.paths.extra_workshop_dirs.iter().cloned());
        out.extend(plan.user_dir.iter().cloned());
        out.sort();
        out.dedup();
        out
    }

    /// The folders the game write fence denies to every project write: the install, its Workshop content
    /// folders, the folders the user named in the settings (the install and user folder overrides and the extra
    /// Workshop folders) and the game's own config and user data folder.
    fn denied_roots(plan: &Plan) -> Vec<Utf8PathBuf> {
        let mut out = Self::protected(plan);
        let paths = &plan.workspace.paths;
        out.extend(paths.game_install.iter().map(|o| o.path.clone()));
        out.extend(paths.user_dir.iter().map(|o| o.path.clone()));
        if let Some(user) = &plan.user_dir {
            out.push(user.join("Config"));
        }
        out.sort();
        out.dedup();
        out
    }

    /// The game write fence of the app (invariant I-05) for the machine the plan describes.
    ///
    /// The fence never writes in the app: the designer and the project tools only ask it to check a path
    /// ([`GameWriteFence::check_project_write`]), which refuses a path inside any denied root even when the
    /// folder lists of the tools would let it pass. Without a game install there is nothing to protect and
    /// the result is `None`. A fence that cannot be built is logged and the folder lists stay the only
    /// defence.
    fn fence_for(app: &AppContext, plan: &Plan) -> Option<Arc<GameWriteFence>> {
        let install = plan.install.as_ref()?;
        let mods_config = plan
            .user_dir
            .as_ref()
            .unwrap_or(&install.game_root)
            .join("Config/ModsConfig.xml");
        let config = FenceConfig {
            install_mods: install.game_root.join("Mods"),
            mods_config,
            protected: Self::denied_roots(plan),
            backup: FenceConfig::mods_config_backup_policy(
                app.roots.data.join("modsconfig-backups"),
            ),
        };
        match GameWriteFence::new(
            config,
            Arc::new(RealFenceFs),
            Arc::clone(&app.platform.links),
            Arc::clone(app.clock()),
        ) {
            Ok(fence) => Some(Arc::new(fence)),
            Err(e) => {
                tracing::warn!(error = %e, "the game write fence could not be built");
                None
            }
        }
    }

    /// The environment that writes new projects, with the protected folders it must honour.
    ///
    /// With built sessions this is the toolkit context's own environment; before that the fence and the
    /// folder list come straight from the plan of the manager, so that `project_create` is fenced too.
    ///
    /// # Errors
    /// The mapped manager error.
    pub fn write_env(&self, app: &AppContext) -> Result<(ProjectEnv, Vec<Utf8PathBuf>), ApiError> {
        if let Some(built) = lock(&self.state).as_ref() {
            return Ok((built.ctx.env().clone(), built.ctx.protected_paths()));
        }
        let plan = Self::plan(app)?;
        let mut env = self.env.clone();
        if let Some(fence) = Self::fence_for(app, &plan) {
            env = env.with_fence(fence);
        }
        Ok((env, Self::protected(&plan)))
    }

    fn open_input(app: &AppContext, reference: ReferenceSet, game_root: &Utf8Path) -> OpenInput {
        let mut input = OpenInput::new(reference);
        input = match &app.options.type_table {
            Some(table) => input.with_type_table(Arc::clone(table)),
            None => input
                .with_game_dir(game_root)
                .with_cache(CacheConfig::in_roots(&app.roots, Arc::clone(app.clock()))),
        };
        let threads = app.settings.get().library.scan_threads;
        if threads > 0 {
            input = input.with_threads(usize::from(threads));
        }
        input
    }

    fn build(&self, app: &AppContext, plan: &Plan) -> Result<DesignerCtx, ApiError> {
        let mut env = self.env.clone().with_protected(Self::protected(plan));
        if let Some(fence) = Self::fence_for(app, plan) {
            env = env.with_fence(fence);
        }
        let clock = Arc::clone(app.clock());
        let toolkit = |e: ToolkitError| app.toolkit_error(&e);
        let Some(install) = &plan.install else {
            tracing::info!("no game install is selected; the toolkit runs without references");
            return DesignerCtx::offline(&app.roots, clock)
                .map(|c| c.with_env(env))
                .map_err(toolkit);
        };
        let version = install
            .version
            .as_ref()
            .and_then(|v| GameVersion::parse(&v.raw).ok());
        let snapshot = app.library.current();
        let (Some(version), Some(snapshot)) = (version, snapshot) else {
            tracing::warn!(
                "the game version or the library is unknown; the toolkit runs without references"
            );
            return DesignerCtx::offline(&app.roots, clock)
                .map(|c| c.with_env(env))
                .map_err(toolkit);
        };
        let reference = ReferenceSet::reference_for_designer(
            snapshot.index.core(),
            &install.game_root,
            &DesignerReferenceOptions::new(version.clone()),
        );
        let input = Self::open_input(app, reference, &install.game_root);
        let session =
            Arc::new(WorkspaceSession::open_simple(input).map_err(|e| app.workspace_error(&e))?);
        let mut ctx = DesignerCtx::new(session, &app.roots, Arc::clone(&clock))
            .map_err(toolkit)?
            .with_env(env);
        if plan.ce_path.is_some() {
            let ce_reference = ReferenceSet::reference_for_designer(
                snapshot.index.core(),
                &install.game_root,
                &DesignerReferenceOptions::new(version).with_mod(CE_PACKAGE_ID),
            );
            let input = Self::open_input(app, ce_reference, &install.game_root)
                .with_custom_ops(custom_registry(&CeClassNames::default(), BTreeMap::new()));
            let ce = Arc::new(
                WorkspaceSession::open_simple(input).map_err(|e| app.workspace_error(&e))?,
            );
            ctx = ctx.with_ce_session(ce);
        }
        Ok(ctx)
    }

    /// The def session a request names.
    ///
    /// # Errors
    /// `defs.session-not-found` for a session that does not exist (no Combat Extended, no such
    /// project), `designer.reference-unavailable` without a game install, and the build errors.
    pub fn session(
        &self,
        app: &AppContext,
        selector: &SessionSelector,
    ) -> Result<Arc<WorkspaceSession>, ApiError> {
        let ctx = self.designer_ctx(app)?;
        match selector {
            SessionSelector::Reference => ctx.session().cloned().ok_or_else(|| {
                ApiError::new(
                    codes::DESIGNER_REFERENCE_UNAVAILABLE,
                    "no game install is loaded",
                )
            }),
            SessionSelector::CombatExtended => ctx.ce_session().cloned().ok_or_else(|| {
                ApiError::new(
                    codes::DEFS_SESSION_NOT_FOUND,
                    "Combat Extended is not part of the library",
                )
                .detail("sessionId", "ce")
            }),
            SessionSelector::Project(id) => {
                let (_record, view) = ctx.env().view(id).map_err(|e| app.toolkit_error(&e))?;
                ctx.project_session(&view.root)
                    .map_err(|e| app.toolkit_error(&e))
            }
        }
    }

    /// Whether Combat Extended is part of the built sessions.
    #[must_use]
    pub fn has_combat_extended(&self) -> bool {
        lock(&self.state)
            .as_ref()
            .is_some_and(|b| b.ctx.ce_session().is_some())
    }
}

/// Loads the workspace document (path overrides and custom folders); a damaged file reads as defaults.
pub(crate) fn load_workspace(app: &AppContext) -> WorkspaceSettings {
    Store::<WorkspaceSettings>::open(
        &app.roots,
        RootKind::Config,
        WORKSPACE_FILE,
        Arc::clone(app.clock()),
    )
    .map(|store| store.load().value)
    .unwrap_or_else(|e| {
        tracing::warn!(error = %e, "the workspace document could not be opened");
        WorkspaceSettings::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_ids_parse() {
        assert_eq!(
            SessionSelector::parse("").ok(),
            Some(SessionSelector::Reference)
        );
        assert_eq!(
            SessionSelector::parse("reference").ok(),
            Some(SessionSelector::Reference)
        );
        assert_eq!(
            SessionSelector::parse("ce").ok(),
            Some(SessionSelector::CombatExtended)
        );
        assert_eq!(
            SessionSelector::parse("project:p-1").ok(),
            Some(SessionSelector::Project("p-1".into()))
        );
    }

    #[test]
    fn unknown_session_ids_are_rejected_with_the_registered_code() {
        for bad in ["nope", "project:", "Project:p-1"] {
            let err = SessionSelector::parse(bad).err();
            assert_eq!(
                err.map(|e| e.code),
                Some("defs.session-not-found".to_owned()),
                "{bad}"
            );
        }
    }
}
