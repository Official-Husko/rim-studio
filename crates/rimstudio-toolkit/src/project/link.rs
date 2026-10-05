//! `project_link_status`, `project_link_create` and `project_link_remove`: make a project visible to the
//! game by linking its folder into `<game install>/Mods`, and take the link away again.
//!
//! The rules (invariant I-05) are in `rimstudio-library::deploy` and the write fence of `rimstudio-io`;
//! this module reads the project, asks the library and turns the answer into the DTOs. It also reads the
//! game's active mod list (`ModsConfig.xml`, read only, through the codec of `rimstudio-xml`) so the page
//! can say whether the project and Combat Extended are enabled.

use camino::Utf8Path;
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::os::Os;
use rimstudio_core::ports::{LinkBackend, LinkSupport, ProcessProbe};
use rimstudio_design::ce::reader::CE_PACKAGE_ID;
use rimstudio_ipc_types::diagnostic::diagnostics_to_dtos;
use rimstudio_ipc_types::project_link::{
    ActiveInGameDto, GameRunningDto, LinkSupportDto, ProjectLinkCreateRequest, ProjectLinkModeDto,
    ProjectLinkRefusalDto, ProjectLinkRemoveRequest, ProjectLinkResultDto, ProjectLinkStateDto,
    ProjectLinkStatusDto, ProjectLinkStatusRequest,
};
use rimstudio_library::deploy::codes;
use rimstudio_library::deploy::inspect::{
    DeployPorts, EntryState, GameRunning, LinkQuery, LinkStatus, inspect,
};
use rimstudio_library::deploy::manifest::{EntryMode, LinkManifest};
use rimstudio_library::deploy::ops::{CreateOptions, DeployOutcome, create, remove};
use rimstudio_xml::mods_config;

use crate::error::ToolkitResult;
use crate::shared::env::ProjectEnv;
use crate::shared::projectfs::ProjectView;

/// What the link commands need from the machine besides the project environment.
#[derive(Clone, Copy)]
pub struct LinkContext<'a> {
    /// The game folder (the one that holds `Mods`), when a game is known.
    pub game_folder: Option<&'a Utf8Path>,
    /// The game's `ModsConfig.xml`, when its folder is known.
    pub mods_config: Option<&'a Utf8Path>,
    /// Directory links.
    pub links: &'a dyn LinkBackend,
    /// Running processes.
    pub probe: &'a dyn ProcessProbe,
    /// The operating system.
    pub os: Os,
    /// The process list is hidden by a sandbox.
    pub hides_processes: bool,
}

fn state_dto(s: EntryState) -> ProjectLinkStateDto {
    match s {
        EntryState::NotLinked => ProjectLinkStateDto::NotLinked,
        EntryState::Linked => ProjectLinkStateDto::Linked,
        EntryState::LinkedByHand => ProjectLinkStateDto::LinkedByHand,
        EntryState::Stale => ProjectLinkStateDto::Stale,
        EntryState::ForeignLink => ProjectLinkStateDto::ForeignLink,
        EntryState::ForeignFolder => ProjectLinkStateDto::ForeignFolder,
        EntryState::Copy => ProjectLinkStateDto::Copy,
        EntryState::InMods => ProjectLinkStateDto::InMods,
        EntryState::Unavailable => ProjectLinkStateDto::Unavailable,
    }
}

fn running_dto(r: GameRunning) -> GameRunningDto {
    match r {
        GameRunning::Running => GameRunningDto::Running,
        GameRunning::NotRunning => GameRunningDto::NotRunning,
        GameRunning::Unknown => GameRunningDto::Unknown,
    }
}

fn mode_dto(m: EntryMode) -> ProjectLinkModeDto {
    match m {
        EntryMode::Symlink => ProjectLinkModeDto::Symlink,
        EntryMode::Junction => ProjectLinkModeDto::Junction,
        EntryMode::Copy => ProjectLinkModeDto::Copy,
    }
}

fn mode_of(m: ProjectLinkModeDto) -> EntryMode {
    match m {
        ProjectLinkModeDto::Symlink => EntryMode::Symlink,
        ProjectLinkModeDto::Junction => EntryMode::Junction,
        ProjectLinkModeDto::Copy => EntryMode::Copy,
    }
}

fn support_dto(s: LinkSupport) -> LinkSupportDto {
    LinkSupportDto {
        symlink: s.symlink,
        junction: s.junction,
        needs_privilege: s.needs_privilege,
    }
}

/// The active ids of the game's `ModsConfig.xml`, lower case; `None` when the file cannot be read.
fn active_ids(path: Option<&Utf8Path>) -> Option<Vec<String>> {
    let bytes = std::fs::read(path?).ok()?;
    let read = mods_config::read(&bytes).ok()?;
    Some(
        read.data
            .active_mods
            .iter()
            .map(|id| id.trim().to_ascii_lowercase())
            .collect(),
    )
}

fn active_state(ids: Option<&[String]>, package_id: Option<&str>) -> ActiveInGameDto {
    match (ids, package_id) {
        (Some(ids), Some(pid)) => {
            let pid = pid.trim().to_ascii_lowercase();
            // The game appends `_steam` to the id of an entry that came from the Workshop copy of a mod.
            if ids
                .iter()
                .any(|id| *id == pid || id.strip_suffix("_steam") == Some(pid.as_str()))
            {
                ActiveInGameDto::Active
            } else {
                ActiveInGameDto::Inactive
            }
        }
        _ => ActiveInGameDto::Unknown,
    }
}

fn base_dto(
    project_id: &str,
    view: &ProjectView,
    ctx: &LinkContext<'_>,
    status: Option<&LinkStatus>,
) -> ProjectLinkStatusDto {
    let ids = active_ids(ctx.mods_config);
    let ids = ids.as_deref();
    let ce_pending = view.gates_on(CE_PACKAGE_ID);
    let support = status.map_or_else(|| ctx.links.support(), |s| s.support);
    ProjectLinkStatusDto {
        project_id: project_id.to_owned(),
        package_id: view.package_id.clone(),
        game_found: ctx.game_folder.is_some(),
        game_folder: ctx.game_folder.map(ToString::to_string),
        mods_folder: status.map(|s| s.mods_folder.to_string()),
        mods_exists: status.is_some_and(|s| s.mods_exists),
        mods_read_only: status.is_some_and(|s| s.mods_read_only),
        link_name: status.map(|s| s.name.clone()).filter(|n| !n.is_empty()),
        entry_path: status
            .filter(|s| !s.name.is_empty())
            .map(|s| s.entry.to_string()),
        state: status.map_or(ProjectLinkStateDto::Unavailable, |s| state_dto(s.state)),
        mode: status.and_then(|s| s.record.as_ref().map(|r| mode_dto(r.mode))),
        points_to: status.and_then(|s| s.points_to.as_ref().map(ToString::to_string)),
        game_running: status.map_or(GameRunningDto::Unknown, |s| running_dto(s.running)),
        support: support_dto(support),
        can_create: status.is_some_and(LinkStatus::can_create),
        can_remove: status.is_some_and(LinkStatus::can_remove),
        manual_command: status
            .map(|s| s.manual_command.clone())
            .filter(|c| !c.is_empty()),
        active_in_game: active_state(ids, view.package_id.as_deref()),
        has_ce_patch: ce_pending,
        ce_active_in_game: active_state(ids, Some(CE_PACKAGE_ID)),
        diagnostics: status
            .map(|s| diagnostics_to_dtos(&s.diagnostics))
            .unwrap_or_default(),
    }
}

fn no_game_dto(
    project_id: &str,
    view: &ProjectView,
    ctx: &LinkContext<'_>,
) -> ProjectLinkStatusDto {
    let mut dto = base_dto(project_id, view, ctx, None);
    dto.diagnostics = diagnostics_to_dtos(&[Diagnostic::new(
        codes::GAME_NOT_FOUND,
        Severity::Warning,
        "no RimWorld install is selected, so there is no Mods folder to link into",
    )]);
    dto
}

struct Prepared<'a> {
    project_id: String,
    view: ProjectView,
    manifest: LinkManifest,
    ctx: LinkContext<'a>,
}

fn prepare<'a>(
    env: &ProjectEnv,
    ctx: LinkContext<'a>,
    project_id: &str,
) -> ToolkitResult<Prepared<'a>> {
    let (record, view) = env.view(project_id)?;
    let manifest = LinkManifest::open(env.roots(), env.clock().clone())?;
    Ok(Prepared {
        project_id: record.id.as_str().to_owned(),
        view,
        manifest,
        ctx,
    })
}

impl Prepared<'_> {
    fn query<'q>(&'q self, env: &'q ProjectEnv) -> Option<LinkQuery<'q>> {
        let fence = env.fence()?;
        Some(LinkQuery {
            fence,
            manifest: &self.manifest,
            ports: DeployPorts {
                links: self.ctx.links,
                probe: self.ctx.probe,
                os: self.ctx.os,
                hides_processes: self.ctx.hides_processes,
            },
            project_root: &self.view.root,
            project_id: Some(&self.project_id),
        })
    }
}

fn result_dto(p: &Prepared<'_>, outcome: DeployOutcome) -> ProjectLinkResultDto {
    ProjectLinkResultDto {
        done: outcome.done,
        refusal: outcome.refusal.map(|r| ProjectLinkRefusalDto {
            code: r.code.as_str().to_owned(),
            message: r.message,
        }),
        status: base_dto(&p.project_id, &p.view, &p.ctx, Some(&outcome.status)),
    }
}

/// `project_link_status`: whether the game can see the project, and what is in the way. Read only.
///
/// # Errors
///
/// [`crate::error::ToolkitError::ProjectNotOpen`] or [`crate::error::ToolkitError::ProjectInvalid`] for
/// the project; a store error when the manifest folder cannot be opened.
pub fn link_status(
    env: &ProjectEnv,
    ctx: LinkContext<'_>,
    req: &ProjectLinkStatusRequest,
) -> ToolkitResult<ProjectLinkStatusDto> {
    let p = prepare(env, ctx, &req.project_id)?;
    let Some(q) = p.query(env) else {
        return Ok(no_game_dto(&p.project_id, &p.view, &p.ctx));
    };
    let status = inspect(&q);
    Ok(base_dto(&p.project_id, &p.view, &p.ctx, Some(&status)))
}

/// `project_link_create`: links the project into the game's `Mods` folder. A refusal (the name is taken,
/// the game is running without confirmation, the folder is read only, ...) is part of the answer.
///
/// # Errors
///
/// As [`link_status`].
pub fn link_create(
    env: &ProjectEnv,
    ctx: LinkContext<'_>,
    req: &ProjectLinkCreateRequest,
) -> ToolkitResult<ProjectLinkResultDto> {
    let p = prepare(env, ctx, &req.project_id)?;
    let Some(q) = p.query(env) else {
        return Ok(ProjectLinkResultDto {
            done: false,
            refusal: Some(ProjectLinkRefusalDto {
                code: codes::GAME_NOT_FOUND.as_str().to_owned(),
                message: "no RimWorld install is selected".to_owned(),
            }),
            status: no_game_dto(&p.project_id, &p.view, &p.ctx),
        });
    };
    let options = CreateOptions {
        mode: mode_of(req.mode),
        confirm_game_running: req.confirm_game_running,
    };
    let outcome = create(&q, options, env.clock().as_ref());
    tracing::info!(done = outcome.done, "project link create");
    Ok(result_dto(&p, outcome))
}

/// `project_link_remove`: removes the link (or marked copy) RimStudio made for the project. Never asks
/// about the running game, never removes anything RimStudio did not create.
///
/// # Errors
///
/// As [`link_status`].
pub fn link_remove(
    env: &ProjectEnv,
    ctx: LinkContext<'_>,
    req: &ProjectLinkRemoveRequest,
) -> ToolkitResult<ProjectLinkResultDto> {
    let p = prepare(env, ctx, &req.project_id)?;
    let Some(q) = p.query(env) else {
        return Ok(ProjectLinkResultDto {
            done: false,
            refusal: Some(ProjectLinkRefusalDto {
                code: codes::GAME_NOT_FOUND.as_str().to_owned(),
                message: "no RimWorld install is selected".to_owned(),
            }),
            status: no_game_dto(&p.project_id, &p.view, &p.ctx),
        });
    };
    let outcome = remove(&q);
    tracing::info!(done = outcome.done, "project link remove");
    Ok(result_dto(&p, outcome))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_active_state_compares_ids_in_any_case() {
        let ids = vec!["ludeon.rimworld".to_owned(), "rs.fiction.arms".to_owned()];
        assert_eq!(
            active_state(Some(&ids), Some("RS.Fiction.Arms")),
            ActiveInGameDto::Active
        );
        assert_eq!(
            active_state(Some(&ids), Some("rs.other")),
            ActiveInGameDto::Inactive
        );
        assert_eq!(active_state(None, Some("x")), ActiveInGameDto::Unknown);
        assert_eq!(active_state(Some(&ids), None), ActiveInGameDto::Unknown);
    }
}
