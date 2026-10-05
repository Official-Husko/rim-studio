//! Reads the state of the entry a project would have in the game `Mods` folder. Writes nothing.

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::os::Os;
use rimstudio_core::ports::{LinkBackend, LinkSupport, ProcessProbe};
use rimstudio_io::fence::{COPY_MARKER, GameWriteFence};
use rimstudio_io::guard::{resolve_path, sanitize_name, to_safe_component};

use super::codes;
use super::command::manual_command;
use super::manifest::{EntryMode, LinkManifest, LinkRecord};

/// What stands at `Mods/<name>`, seen from the project.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryState {
    /// Nothing is there: the project can be linked.
    NotLinked,
    /// A link RimStudio created, pointing at the project: the game sees the mod.
    Linked,
    /// A link that RimStudio did not create but that points at the project: the game sees the mod, and
    /// RimStudio will not remove it.
    LinkedByHand,
    /// A link RimStudio created whose target is not the project any more (the project moved) or is gone.
    Stale,
    /// A link that RimStudio did not create and that points somewhere else.
    ForeignLink,
    /// A real folder or file that RimStudio did not create.
    ForeignFolder,
    /// A marked copy RimStudio created.
    Copy,
    /// The project folder itself is inside `Mods`: the game sees it already.
    InMods,
    /// There is nothing to link into or to link (no game, no `Mods` folder, no project folder).
    Unavailable,
}

impl EntryState {
    /// The name on the wire.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            EntryState::NotLinked => "not-linked",
            EntryState::Linked => "linked",
            EntryState::LinkedByHand => "linked-by-hand",
            EntryState::Stale => "stale",
            EntryState::ForeignLink => "foreign-link",
            EntryState::ForeignFolder => "foreign-folder",
            EntryState::Copy => "copy",
            EntryState::InMods => "in-mods",
            EntryState::Unavailable => "unavailable",
        }
    }
}

/// Whether the game is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameRunning {
    /// A game process was found.
    Running,
    /// No game process was found.
    NotRunning,
    /// The process table is hidden (a sandbox), so the answer is not known.
    Unknown,
}

impl GameRunning {
    /// The name on the wire.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            GameRunning::Running => "running",
            GameRunning::NotRunning => "not-running",
            GameRunning::Unknown => "unknown",
        }
    }
}

/// The ports a deploy call needs besides the fence.
#[derive(Clone, Copy)]
pub struct DeployPorts<'a> {
    /// Directory links (read side here, the fence owns the writes).
    pub links: &'a dyn LinkBackend,
    /// Running processes.
    pub probe: &'a dyn ProcessProbe,
    /// The operating system.
    pub os: Os,
    /// True when the process table is hidden by a sandbox.
    pub hides_processes: bool,
}

/// What a deploy call is about.
#[derive(Clone, Copy)]
pub struct LinkQuery<'a> {
    /// The write fence of the install (it knows the `Mods` folder).
    pub fence: &'a GameWriteFence,
    /// The ownership manifest.
    pub manifest: &'a LinkManifest,
    /// The ports.
    pub ports: DeployPorts<'a>,
    /// The project folder.
    pub project_root: &'a Utf8Path,
    /// The project id, recorded in the manifest.
    pub project_id: Option<&'a str>,
}

/// Everything known about the entry.
#[derive(Debug, Clone)]
pub struct LinkStatus {
    /// The `Mods` folder of the install.
    pub mods_folder: Utf8PathBuf,
    /// The `Mods` folder exists.
    pub mods_exists: bool,
    /// The `Mods` folder carries a read only flag.
    pub mods_read_only: bool,
    /// The entry name (the project folder name, made safe).
    pub name: String,
    /// The full path of the entry.
    pub entry: Utf8PathBuf,
    /// The canonical project folder, when it exists.
    pub target: Option<Utf8PathBuf>,
    /// The state.
    pub state: EntryState,
    /// Where an existing link points.
    pub points_to: Option<Utf8PathBuf>,
    /// The manifest record of the entry, when there is one.
    pub record: Option<LinkRecord>,
    /// Whether the game is running.
    pub running: GameRunning,
    /// What the link backend can create.
    pub support: LinkSupport,
    /// The command a person can run to create the link by hand.
    pub manual_command: String,
    /// Findings, most important first.
    pub diagnostics: Vec<Diagnostic>,
}

/// The entry name of a project folder: its folder name made safe for one path component. A name that
/// starts with a dot gets a leading underscore (the fence refuses hidden names).
#[must_use]
pub fn link_name(project_root: &Utf8Path) -> Option<String> {
    let raw = project_root.file_name()?;
    let mut name = to_safe_component(raw);
    if name.starts_with('.') {
        name.insert(0, '_');
    }
    sanitize_name(&name).ok()
}

fn same_path(a: &Utf8Path, b: &Utf8Path) -> bool {
    if a == b {
        return true;
    }
    match (resolve_path(a), resolve_path(b)) {
        (Ok(x), Ok(y)) => x == y,
        _ => false,
    }
}

fn running_state(ports: &DeployPorts<'_>) -> GameRunning {
    let found = ports.probe.running(ports.os.game_executable_names());
    if !found.is_empty() {
        GameRunning::Running
    } else if ports.hides_processes {
        GameRunning::Unknown
    } else {
        GameRunning::NotRunning
    }
}

fn marker_is_ours(entry: &Utf8Path) -> bool {
    fs_err::read(entry.join(COPY_MARKER))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|v| v.get("owner").and_then(|o| o.as_str().map(str::to_owned)))
        .is_some_and(|o| o == "rimstudio")
}

fn warn(code: rimstudio_core::diag::DiagCode, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, Severity::Warning, message)
}

fn info(code: rimstudio_core::diag::DiagCode, message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(code, Severity::Info, message)
}

/// Reads the state of the entry for a project. Never writes.
#[must_use]
pub fn inspect(q: &LinkQuery<'_>) -> LinkStatus {
    let mods = q.fence.install_mods().to_path_buf();
    let mut diagnostics = Vec::new();
    let name = link_name(q.project_root).unwrap_or_default();
    let entry = mods.join(&name);
    let mods_meta = fs_err::metadata(&mods)
        .ok()
        .filter(std::fs::Metadata::is_dir);
    let mods_exists = mods_meta.is_some();
    let mods_read_only = mods_meta.is_some_and(|m| m.permissions().readonly());
    let target = resolve_path(q.project_root).ok().filter(|t| t.is_dir());
    let record = match q.manifest.get(&mods, &name) {
        Ok(r) => r,
        Err(e) => {
            diagnostics.push(warn(
                codes::MANIFEST,
                format!("the ownership record could not be read: {e}"),
            ));
            None
        }
    };
    let mut status = LinkStatus {
        manual_command: target
            .as_deref()
            .map(|t| manual_command(q.ports.os, t, &entry))
            .unwrap_or_default(),
        mods_folder: mods.clone(),
        mods_exists,
        mods_read_only,
        name: name.clone(),
        entry: entry.clone(),
        target: target.clone(),
        state: EntryState::Unavailable,
        points_to: None,
        record,
        running: running_state(&q.ports),
        support: q.ports.links.support(),
        diagnostics,
    };
    let Some(target) = target else {
        status.diagnostics.push(warn(
            codes::TARGET_REFUSED,
            "the project folder does not exist or is not a folder",
        ));
        return status;
    };
    if name.is_empty() {
        status.diagnostics.push(warn(
            codes::TARGET_REFUSED,
            "the project folder has no usable name to link under",
        ));
        return status;
    }
    if !mods_exists {
        status.diagnostics.push(warn(
            codes::MODS_MISSING,
            "the game's Mods folder does not exist",
        ));
        return status;
    }
    if resolve_path(&mods).is_ok_and(|m| target.parent() == Some(m.as_path())) {
        status.state = EntryState::InMods;
        status.diagnostics.push(info(
            codes::IN_MODS,
            "the project folder is inside the game's Mods folder, so the game already sees it",
        ));
        return status;
    }
    classify(q, &mut status, &target);
    status
}

fn classify(q: &LinkQuery<'_>, status: &mut LinkStatus, target: &Utf8Path) {
    let entry = status.entry.clone();
    let meta = match fs_err::symlink_metadata(&entry) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            status.state = EntryState::NotLinked;
            return;
        }
        Err(e) => {
            status.diagnostics.push(warn(
                codes::IO,
                format!("the entry in the Mods folder cannot be inspected: {e}"),
            ));
            return;
        }
    };
    if meta.file_type().is_symlink() || q.ports.links.is_link(&entry) {
        let raw = q.ports.links.read_link(&entry);
        let points_to = raw.map(|p| {
            if p.is_relative() {
                status.mods_folder.join(p)
            } else {
                p
            }
        });
        let recorded = status.record.as_ref().filter(|r| {
            r.mode.is_link()
                && points_to
                    .as_deref()
                    .is_some_and(|p| same_path(&r.target, p))
        });
        let points_at_project = points_to.as_deref().is_some_and(|p| same_path(p, target));
        status.state = match (recorded.is_some(), points_at_project) {
            (true, true) => EntryState::Linked,
            (true, false) => EntryState::Stale,
            (false, true) => EntryState::LinkedByHand,
            (false, false) => EntryState::ForeignLink,
        };
        status.points_to = points_to;
    } else if meta.is_dir()
        && status
            .record
            .as_ref()
            .is_some_and(|r| r.mode == EntryMode::Copy)
        && marker_is_ours(&entry)
    {
        status.state = EntryState::Copy;
    } else {
        status.state = EntryState::ForeignFolder;
    }
    match status.state {
        EntryState::ForeignFolder | EntryState::ForeignLink => status.diagnostics.push(warn(
            codes::NAME_TAKEN,
            "something with this name already exists in the Mods folder and was not made by RimStudio",
        )),
        EntryState::LinkedByHand => status.diagnostics.push(info(
            codes::ALREADY_LINKED,
            "a link to this project already exists in the Mods folder; RimStudio did not make it and will not remove it",
        )),
        _ => {}
    }
}

impl LinkStatus {
    /// True when `create` may be attempted: the entry is free and the project is linkable.
    #[must_use]
    pub fn can_create(&self) -> bool {
        self.state == EntryState::NotLinked
    }

    /// True when `remove` may be attempted: the entry is one RimStudio made.
    #[must_use]
    pub fn can_remove(&self) -> bool {
        matches!(
            self.state,
            EntryState::Linked | EntryState::Stale | EntryState::Copy
        )
    }
}
