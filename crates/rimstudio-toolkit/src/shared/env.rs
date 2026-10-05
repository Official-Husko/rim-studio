//! The project environment: where projects are registered, which folders are never written, and how a
//! guarded writer for one project is made.
//!
//! [`ProjectEnv`] is shared by the designer's context and by the project tool, so the two never use each
//! other. It holds the project store (the JSON documents of registered projects in the app data root), the
//! protected folders (the game install and the game config folder), the optional write fence of the app and
//! the clock for backups.

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ids::ProjectId;
use rimstudio_core::ports::Clock;
use rimstudio_io::fence::GameWriteFence;
use rimstudio_io::roots::DataRoots;
use rimstudio_workspace::project::{ProjectRecord, ProjectStore};

use super::projectfs::{ProjectView, read_project};
use super::writer::GuardedWriter;
use crate::error::{ToolkitError, ToolkitResult};

/// The folder (below the data root) that holds the backups of files the toolkit replaced in projects.
pub const BACKUP_FOLDER: &str = "project-backups";

/// Shared state of the tools that work on mod projects.
#[derive(Clone)]
pub struct ProjectEnv {
    roots: DataRoots,
    clock: Arc<dyn Clock>,
    projects: ProjectStore,
    protected: Vec<Utf8PathBuf>,
    fence: Option<Arc<GameWriteFence>>,
}

impl std::fmt::Debug for ProjectEnv {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectEnv")
            .field("protected", &self.protected)
            .field("has_fence", &self.fence.is_some())
            .finish_non_exhaustive()
    }
}

impl ProjectEnv {
    /// An environment over the data roots; the project store lives in the data root.
    ///
    /// # Errors
    ///
    /// [`ToolkitError::Workspace`] when the store folder cannot be created.
    pub fn new(roots: &DataRoots, clock: Arc<dyn Clock>) -> ToolkitResult<Self> {
        let projects = ProjectStore::open_in(roots, clock.clone())?;
        Ok(Self {
            roots: roots.clone(),
            clock,
            projects,
            protected: Vec::new(),
            fence: None,
        })
    }

    /// The same environment with more protected folders (never written, whatever the project root is).
    #[must_use]
    pub fn with_protected(mut self, folders: impl IntoIterator<Item = Utf8PathBuf>) -> Self {
        self.protected.extend(folders);
        self
    }

    /// The same environment with the game write fence of the app.
    #[must_use]
    pub fn with_fence(mut self, fence: Arc<GameWriteFence>) -> Self {
        self.fence = Some(fence);
        self
    }

    /// The game write fence this environment checks every project write against, when the app installed one.
    #[must_use]
    pub fn fence(&self) -> Option<&Arc<GameWriteFence>> {
        self.fence.as_ref()
    }

    /// The data roots.
    #[must_use]
    pub fn roots(&self) -> &DataRoots {
        &self.roots
    }

    /// The clock.
    #[must_use]
    pub fn clock(&self) -> &Arc<dyn Clock> {
        &self.clock
    }

    /// The store of registered projects.
    #[must_use]
    pub fn projects(&self) -> &ProjectStore {
        &self.projects
    }

    /// The explicitly protected folders.
    #[must_use]
    pub fn protected(&self) -> &[Utf8PathBuf] {
        &self.protected
    }

    /// The registered project with this id.
    ///
    /// # Errors
    ///
    /// [`ToolkitError::ProjectNotOpen`] for an unknown or malformed id.
    pub fn record(&self, project_id: &str) -> ToolkitResult<ProjectRecord> {
        let not_open = || ToolkitError::ProjectNotOpen {
            id: project_id.chars().take(80).collect(),
        };
        let id = ProjectId::new(project_id).map_err(|_| not_open())?;
        self.projects.get(&id)?.ok_or_else(not_open)
    }

    /// The read only view of the registered project with this id.
    ///
    /// # Errors
    ///
    /// [`ToolkitError::ProjectNotOpen`], or [`ToolkitError::ProjectInvalid`] when the folder is no longer a mod.
    pub fn view(&self, project_id: &str) -> ToolkitResult<(ProjectRecord, ProjectView)> {
        let record = self.record(project_id)?;
        let view = read_project(&record.path)?;
        Ok((record, view))
    }

    /// A guarded writer for a project root. `extra_protected` adds folders to the protected list (the
    /// designer adds the roots of the reference packs).
    ///
    /// # Errors
    ///
    /// [`ToolkitError::PathRefused`] when the root lies inside a protected folder.
    pub fn writer(
        &self,
        root: &Utf8Path,
        project_id: &str,
        extra_protected: &[Utf8PathBuf],
    ) -> ToolkitResult<GuardedWriter> {
        let mut protected = self.protected.clone();
        protected.extend(extra_protected.iter().cloned());
        let backups = self
            .roots
            .data
            .join(BACKUP_FOLDER)
            .join(rimstudio_io::guard::to_safe_component(project_id));
        GuardedWriter::new(
            root,
            &protected,
            backups,
            self.fence.clone(),
            self.clock.clone(),
        )
    }
}
