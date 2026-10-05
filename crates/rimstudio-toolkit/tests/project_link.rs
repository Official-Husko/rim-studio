//! The link commands of the project tool on a fiction install in a temporary folder with real symbolic
//! links: status, create, remove, the refusals as values, the active mod list hint and the Combat
//! Extended hints. Names are fictional. Unix only (the test link backend makes symlinks).
#![cfg(all(unix, feature = "tool-project"))]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::os::Os;
use rimstudio_core::ports::{
    LinkBackend, LinkKind, LinkSupport, PortError, PortErrorKind, PortResult,
};
use rimstudio_io::fence::{FenceConfig, GameWriteFence, RealFenceFs};
use rimstudio_io::roots::DataRoots;
use rimstudio_ipc_types::project_link::{
    ActiveInGameDto, GameRunningDto, ProjectLinkCreateRequest, ProjectLinkModeDto,
    ProjectLinkRemoveRequest, ProjectLinkStateDto, ProjectLinkStatusRequest,
};
use rimstudio_testing::fakes::{FakeClock, FakeProcessProbe};
use rimstudio_toolkit::error::ToolkitError;
use rimstudio_toolkit::project::link::{LinkContext, link_create, link_remove, link_status};
use rimstudio_toolkit::project::open_project;
use rimstudio_toolkit::shared::env::ProjectEnv;

struct DiskLinks;

impl LinkBackend for DiskLinks {
    fn support(&self) -> LinkSupport {
        LinkSupport {
            symlink: true,
            junction: false,
            needs_privilege: false,
        }
    }
    fn create_dir_link(&self, target: &Utf8Path, link: &Utf8Path) -> PortResult<LinkKind> {
        std::os::unix::fs::symlink(target, link)
            .map_err(|e| PortError::new(PortErrorKind::Io, e.to_string()))?;
        Ok(LinkKind::Symlink)
    }
    fn remove_link(&self, link: &Utf8Path) -> PortResult<()> {
        std::fs::remove_file(link).map_err(|e| PortError::new(PortErrorKind::Io, e.to_string()))
    }
    fn is_link(&self, path: &Utf8Path) -> bool {
        std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink())
    }
    fn read_link(&self, path: &Utf8Path) -> Option<Utf8PathBuf> {
        std::fs::read_link(path)
            .ok()
            .and_then(|p| Utf8PathBuf::from_path_buf(p).ok())
    }
}

struct World {
    _tmp: tempfile::TempDir,
    base: Utf8PathBuf,
    install: Utf8PathBuf,
    mods: Utf8PathBuf,
    config: Utf8PathBuf,
    env: ProjectEnv,
    probe: FakeProcessProbe,
    project: Utf8PathBuf,
    project_id: String,
}

const ABOUT: &str = "<ModMetaData><name>RS Arms</name><packageId>RS.Fiction.Arms</packageId><supportedVersions><li>1.6</li></supportedVersions></ModMetaData>";

fn world(with_fence: bool) -> World {
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().canonicalize().unwrap()).unwrap();
    let install = base.join("RimWorld");
    let mods = install.join("Mods");
    let config = base.join("GameConfig");
    for d in [&mods, &config, &install.join("Data")] {
        std::fs::create_dir_all(d).unwrap();
    }
    let roots = DataRoots::under_base(&base.join("app"));
    roots.ensure_all().unwrap();
    let clock = Arc::new(FakeClock::new(FakeClock::DEFAULT_START_MS));
    let mut env = ProjectEnv::new(&roots, clock.clone()).unwrap();
    if with_fence {
        let fence = GameWriteFence::new(
            FenceConfig {
                install_mods: mods.clone(),
                mods_config: config.join("ModsConfig.xml"),
                protected: vec![install.clone(), config.clone()],
                backup: FenceConfig::mods_config_backup_policy(base.join("app/backups")),
            },
            Arc::new(RealFenceFs),
            Arc::new(DiskLinks),
            clock,
        )
        .unwrap();
        env = env.with_fence(Arc::new(fence));
    }
    let project = base.join("MyMods/RS_Arms");
    std::fs::create_dir_all(project.join("About")).unwrap();
    std::fs::write(project.join("About/About.xml"), ABOUT).unwrap();
    let project_id = open_project(&env, &project)
        .unwrap()
        .record
        .id
        .as_str()
        .to_owned();
    World {
        _tmp: tmp,
        base,
        install,
        mods,
        config,
        env,
        probe: FakeProcessProbe::new(),
        project,
        project_id,
    }
}

impl World {
    fn ctx(&self) -> LinkContext<'_> {
        LinkContext {
            game_folder: Some(&self.install),
            mods_config: None,
            links: &DiskLinks,
            probe: &self.probe,
            os: Os::Linux,
            hides_processes: false,
        }
    }
    fn ctx_with_config<'a>(&'a self, config: &'a Utf8Path) -> LinkContext<'a> {
        LinkContext {
            mods_config: Some(config),
            ..self.ctx()
        }
    }
}

fn status_req(w: &World) -> ProjectLinkStatusRequest {
    ProjectLinkStatusRequest {
        project_id: w.project_id.clone(),
    }
}

fn create_req(w: &World) -> ProjectLinkCreateRequest {
    ProjectLinkCreateRequest {
        project_id: w.project_id.clone(),
        ..ProjectLinkCreateRequest::default()
    }
}

#[test]
fn the_whole_cycle_through_the_commands() {
    let w = world(true);
    let s = link_status(&w.env, w.ctx(), &status_req(&w)).unwrap();
    assert_eq!(s.state, ProjectLinkStateDto::NotLinked);
    assert!(s.game_found && s.mods_exists && s.can_create && !s.can_remove);
    assert_eq!(s.link_name.as_deref(), Some("RS_Arms"));
    assert_eq!(s.package_id.as_deref(), Some("RS.Fiction.Arms"));
    assert_eq!(s.game_running, GameRunningDto::NotRunning);
    assert!(s.manual_command.as_deref().unwrap().starts_with("ln -s "));

    let made = link_create(&w.env, w.ctx(), &create_req(&w)).unwrap();
    assert!(made.done, "{:?}", made.refusal);
    assert_eq!(made.status.state, ProjectLinkStateDto::Linked);
    assert_eq!(made.status.mode, Some(ProjectLinkModeDto::Symlink));
    assert_eq!(made.status.points_to.as_deref(), Some(w.project.as_str()));
    assert!(w.mods.join("RS_Arms/About/About.xml").is_file());

    let again = link_create(&w.env, w.ctx(), &create_req(&w)).unwrap();
    assert!(!again.done);
    assert_eq!(again.refusal.unwrap().code, "deploy.already-linked");

    let gone = link_remove(
        &w.env,
        w.ctx(),
        &ProjectLinkRemoveRequest {
            project_id: w.project_id.clone(),
        },
    )
    .unwrap();
    assert!(gone.done, "{:?}", gone.refusal);
    assert_eq!(gone.status.state, ProjectLinkStateDto::NotLinked);
    assert!(!w.mods.join("RS_Arms").exists());
    assert!(w.project.join("About/About.xml").is_file());
}

#[test]
fn a_refusal_is_a_value_with_the_reason_and_the_manual_command() {
    let w = world(true);
    std::fs::create_dir_all(w.mods.join("RS_Arms")).unwrap();
    let out = link_create(&w.env, w.ctx(), &create_req(&w)).unwrap();
    assert!(!out.done);
    let r = out.refusal.unwrap();
    assert_eq!(r.code, "deploy.name-taken");
    assert_eq!(out.status.state, ProjectLinkStateDto::ForeignFolder);
    assert!(out.status.manual_command.is_some());
    assert!(
        out.status
            .diagnostics
            .iter()
            .any(|d| d.code == "deploy.name-taken")
    );
}

#[test]
fn a_running_game_needs_a_confirmation_to_create() {
    let w = world(true);
    w.probe.add(7, "RimWorldLinux", None);
    let s = link_status(&w.env, w.ctx(), &status_req(&w)).unwrap();
    assert_eq!(s.game_running, GameRunningDto::Running);
    let blocked = link_create(&w.env, w.ctx(), &create_req(&w)).unwrap();
    assert_eq!(blocked.refusal.unwrap().code, "deploy.game-running");
    let ok = link_create(
        &w.env,
        w.ctx(),
        &ProjectLinkCreateRequest {
            confirm_game_running: true,
            ..create_req(&w)
        },
    )
    .unwrap();
    assert!(ok.done);
    let gone = link_remove(
        &w.env,
        w.ctx(),
        &ProjectLinkRemoveRequest {
            project_id: w.project_id.clone(),
        },
    )
    .unwrap();
    assert!(gone.done, "remove is never blocked by a running game");
}

#[test]
fn without_a_game_install_the_answer_says_so_and_nothing_is_written() {
    let w = world(false);
    let ctx = LinkContext {
        game_folder: None,
        ..w.ctx()
    };
    let s = link_status(&w.env, ctx, &status_req(&w)).unwrap();
    assert!(!s.game_found);
    assert_eq!(s.state, ProjectLinkStateDto::Unavailable);
    assert!(!s.can_create);
    assert!(
        s.diagnostics
            .iter()
            .any(|d| d.code == "deploy.game-not-found")
    );
    let out = link_create(&w.env, ctx, &create_req(&w)).unwrap();
    assert!(!out.done);
    assert_eq!(out.refusal.unwrap().code, "deploy.game-not-found");
    assert!(std::fs::read_dir(&w.mods).unwrap().next().is_none());
}

#[test]
fn an_unknown_project_is_an_error_not_a_refusal() {
    let w = world(true);
    let err = link_status(
        &w.env,
        w.ctx(),
        &ProjectLinkStatusRequest {
            project_id: "p-00000000".into(),
        },
    )
    .unwrap_err();
    assert!(matches!(err, ToolkitError::ProjectNotOpen { .. }));
}

#[test]
fn the_active_mod_list_says_whether_the_project_and_combat_extended_are_enabled() {
    let w = world(true);
    let config = w.config.join("ModsConfig.xml");
    std::fs::write(
        &config,
        "<ModsConfigData><version>1.6.4871 rev598</version><activeMods><li>ludeon.rimworld</li><li>rs.fiction.arms</li></activeMods></ModsConfigData>",
    )
    .unwrap();
    let before = std::fs::read(&config).unwrap();
    let s = link_status(&w.env, w.ctx_with_config(&config), &status_req(&w)).unwrap();
    assert_eq!(s.active_in_game, ActiveInGameDto::Active);
    assert_eq!(s.ce_active_in_game, ActiveInGameDto::Inactive);
    assert!(!s.has_ce_patch);
    assert_eq!(
        std::fs::read(&config).unwrap(),
        before,
        "the file is only read"
    );

    let s = link_status(&w.env, w.ctx(), &status_req(&w)).unwrap();
    assert_eq!(s.active_in_game, ActiveInGameDto::Unknown);
    let missing = w.config.join("Missing.xml");
    let s = link_status(&w.env, w.ctx_with_config(&missing), &status_req(&w)).unwrap();
    assert_eq!(s.active_in_game, ActiveInGameDto::Unknown);
}

#[test]
fn a_project_with_a_gated_combat_extended_folder_is_marked() {
    let w = world(true);
    std::fs::write(
        w.project.join("LoadFolders.xml"),
        "<loadFolders><v1.6><li>/</li><li IfModActive=\"ceteam.combatextended\">Compat/CombatExtended</li></v1.6></loadFolders>",
    )
    .unwrap();
    let s = link_status(&w.env, w.ctx(), &status_req(&w)).unwrap();
    assert!(s.has_ce_patch);
    let _ = &w.base;
}
