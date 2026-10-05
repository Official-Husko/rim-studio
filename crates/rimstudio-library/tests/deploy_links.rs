//! The link of a project into a game `Mods` folder, on a fiction install in a temporary folder with
//! real symbolic links. Names are fictional. Unix only (the link backend of the test makes symlinks).
#![cfg(unix)]
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::os::Os;
use rimstudio_core::ports::{
    Clock, LinkBackend, LinkKind, LinkSupport, PortError, PortErrorKind, PortResult,
};
use rimstudio_io::fence::{FenceConfig, GameWriteFence, RealFenceFs};
use rimstudio_library::deploy::codes;
use rimstudio_library::deploy::inspect::{
    DeployPorts, EntryState, GameRunning, LinkQuery, inspect, link_name,
};
use rimstudio_library::deploy::manifest::{EntryMode, LinkManifest};
use rimstudio_library::deploy::ops::{CreateOptions, create, remove};
use rimstudio_testing::fakes::{FakeClock, FakeProcessProbe};

struct DiskLinks {
    support: LinkSupport,
    fail_create: bool,
}

impl DiskLinks {
    fn normal() -> Self {
        DiskLinks {
            support: LinkSupport {
                symlink: true,
                junction: false,
                needs_privilege: false,
            },
            fail_create: false,
        }
    }
}

impl LinkBackend for DiskLinks {
    fn support(&self) -> LinkSupport {
        self.support
    }
    fn create_dir_link(&self, target: &Utf8Path, link: &Utf8Path) -> PortResult<LinkKind> {
        if self.fail_create {
            return Err(PortError::new(
                PortErrorKind::Io,
                "Permission denied (os error 13)",
            ));
        }
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
    _t: tempfile::TempDir,
    base: Utf8PathBuf,
    install: Utf8PathBuf,
    mods: Utf8PathBuf,
    project: Utf8PathBuf,
    fence: GameWriteFence,
    manifest: LinkManifest,
    links: DiskLinks,
    probe: FakeProcessProbe,
    clock: FakeClock,
}

fn world_with(links: DiskLinks) -> World {
    let t = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(std::fs::canonicalize(t.path()).unwrap()).unwrap();
    let install = base.join("RimWorld");
    let mods = install.join("Mods");
    let project = base.join("MyMods/RS_Fiction");
    for d in [
        &mods,
        &install.join("Data/Core"),
        &base.join("GameConfig"),
        &project.join("About"),
    ] {
        fs_err::create_dir_all(d).unwrap();
    }
    fs_err::write(project.join("About/About.xml"), "<ModMetaData/>").unwrap();
    fs_err::write(install.join("Version.txt"), "1.6.0").unwrap();
    let clock = FakeClock::new(1_700_000_000_000);
    let fence = GameWriteFence::new(
        FenceConfig {
            install_mods: mods.clone(),
            mods_config: base.join("GameConfig/ModsConfig.xml"),
            protected: vec![install.clone(), base.join("GameConfig")],
            backup: FenceConfig::mods_config_backup_policy(base.join("backups")),
        },
        Arc::new(RealFenceFs),
        Arc::new(DiskLinks {
            support: links.support,
            fail_create: links.fail_create,
        }),
        Arc::new(clock.clone()),
    )
    .unwrap();
    let manifest =
        LinkManifest::open_dir(base.join("data/game-links"), Arc::new(clock.clone())).unwrap();
    World {
        _t: t,
        base,
        install,
        mods,
        project,
        fence,
        manifest,
        links,
        probe: FakeProcessProbe::new(),
        clock,
    }
}

fn world() -> World {
    world_with(DiskLinks::normal())
}

impl World {
    fn query<'a>(&'a self, root: &'a Utf8Path) -> LinkQuery<'a> {
        LinkQuery {
            fence: &self.fence,
            manifest: &self.manifest,
            ports: DeployPorts {
                links: &self.links,
                probe: &self.probe,
                os: Os::Linux,
                hides_processes: false,
            },
            project_root: root,
            project_id: Some("p-test"),
        }
    }
    fn q(&self) -> LinkQuery<'_> {
        self.query(&self.project)
    }
}

const SYMLINK: CreateOptions = CreateOptions {
    mode: EntryMode::Symlink,
    confirm_game_running: false,
};

fn tree(root: &Utf8Path) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(dir: &Utf8Path, root: &Utf8Path, out: &mut Vec<String>) {
        let mut entries: Vec<_> = fs_err::read_dir(dir).unwrap().map(Result::unwrap).collect();
        entries.sort_by_key(fs_err::DirEntry::file_name);
        for e in entries {
            let p = Utf8PathBuf::from_path_buf(e.path()).unwrap();
            let rel = p.strip_prefix(root).unwrap().to_string();
            let ft = e.file_type().unwrap();
            if ft.is_symlink() {
                out.push(format!("link {rel}"));
            } else if ft.is_dir() {
                out.push(format!("dir {rel}"));
                walk(&p, root, out);
            } else {
                out.push(format!("file {rel}"));
            }
        }
    }
    walk(root, root, &mut out);
    out
}

#[test]
fn a_project_is_linked_seen_and_unlinked_and_the_project_is_never_touched() {
    let w = world();
    let project_before = tree(&w.project);
    let s = inspect(&w.q());
    assert_eq!(s.state, EntryState::NotLinked);
    assert!(s.can_create() && !s.can_remove());
    assert_eq!(s.name, "RS_Fiction");
    assert!(s.manual_command.starts_with("ln -s '"));
    assert!(s.manual_command.ends_with("/Mods/RS_Fiction'"));

    let out = create(&w.q(), SYMLINK, &w.clock);
    assert!(out.done, "{:?}", out.refusal);
    assert_eq!(out.status.state, EntryState::Linked);
    assert_eq!(out.status.points_to.as_deref(), Some(w.project.as_path()));
    assert_eq!(tree(&w.mods), ["link RS_Fiction"]);
    let record = w.manifest.get(&w.mods, "RS_Fiction").unwrap().unwrap();
    assert_eq!(record.target, w.project);
    assert_eq!(record.created_ms, 1_700_000_000_000);
    assert_eq!(record.project_id.as_deref(), Some("p-test"));
    // The game sees the files through the link.
    assert!(w.mods.join("RS_Fiction/About/About.xml").is_file());

    let again = create(&w.q(), SYMLINK, &w.clock);
    assert!(!again.done);
    assert_eq!(again.refusal.unwrap().code, codes::ALREADY_LINKED);

    let gone = remove(&w.q());
    assert!(gone.done, "{:?}", gone.refusal);
    assert_eq!(gone.status.state, EntryState::NotLinked);
    assert!(tree(&w.mods).is_empty());
    assert!(w.manifest.get(&w.mods, "RS_Fiction").unwrap().is_none());
    assert_eq!(
        tree(&w.project),
        project_before,
        "the sentinel tree is intact"
    );
}

#[test]
fn nothing_that_exists_is_ever_replaced_or_removed() {
    let w = world();
    fs_err::create_dir_all(w.mods.join("RS_Fiction/inner")).unwrap();
    fs_err::write(w.mods.join("RS_Fiction/inner/keep.txt"), "keep").unwrap();
    let before = tree(&w.install);
    assert_eq!(inspect(&w.q()).state, EntryState::ForeignFolder);
    let out = create(&w.q(), SYMLINK, &w.clock);
    assert!(!out.done);
    assert_eq!(out.refusal.unwrap().code, codes::NAME_TAKEN);
    let out = remove(&w.q());
    assert!(!out.done);
    assert_eq!(out.refusal.unwrap().code, codes::NOT_OURS);
    assert_eq!(tree(&w.install), before);
    assert!(w.manifest.list_for(&w.mods).unwrap().is_empty());
}

#[test]
fn a_foreign_link_elsewhere_and_a_hand_made_link_to_the_project() {
    let w = world();
    let elsewhere = w.base.join("Elsewhere");
    fs_err::create_dir_all(&elsewhere).unwrap();
    std::os::unix::fs::symlink(&elsewhere, w.mods.join("RS_Fiction")).unwrap();
    let s = inspect(&w.q());
    assert_eq!(s.state, EntryState::ForeignLink);
    assert_eq!(s.points_to.as_deref(), Some(elsewhere.as_path()));
    assert!(!create(&w.q(), SYMLINK, &w.clock).done);
    assert!(!remove(&w.q()).done);
    assert!(w.mods.join("RS_Fiction").is_symlink());

    std::fs::remove_file(w.mods.join("RS_Fiction")).unwrap();
    std::os::unix::fs::symlink(&w.project, w.mods.join("RS_Fiction")).unwrap();
    let s = inspect(&w.q());
    assert_eq!(s.state, EntryState::LinkedByHand);
    let r = remove(&w.q());
    assert!(!r.done, "a link RimStudio did not make is not removed");
    assert!(w.mods.join("RS_Fiction").is_symlink());
}

#[test]
fn a_stale_link_is_listed_and_can_be_removed_but_not_replaced() {
    let w = world();
    assert!(create(&w.q(), SYMLINK, &w.clock).done);
    // The project moved: the same folder name now lives elsewhere and the old folder is gone.
    let moved = w.base.join("Moved/RS_Fiction");
    fs_err::create_dir_all(moved.join("About")).unwrap();
    fs_err::rename(&w.project, w.base.join("Gone")).unwrap();
    let s = inspect(&w.query(&moved));
    assert_eq!(s.state, EntryState::Stale, "{s:?}");
    assert!(s.can_remove() && !s.can_create());
    let c = create(&w.query(&moved), SYMLINK, &w.clock);
    assert!(!c.done);
    let r = remove(&w.query(&moved));
    assert!(r.done, "{:?}", r.refusal);
    assert_eq!(r.status.state, EntryState::NotLinked);
    assert!(w.base.join("Gone/About/About.xml").is_file());
}

#[test]
fn a_link_that_was_retargeted_after_we_made_it_is_foreign_and_stays() {
    let w = world();
    assert!(create(&w.q(), SYMLINK, &w.clock).done);
    let other = w.base.join("Other");
    fs_err::create_dir_all(&other).unwrap();
    std::fs::remove_file(w.mods.join("RS_Fiction")).unwrap();
    std::os::unix::fs::symlink(&other, w.mods.join("RS_Fiction")).unwrap();
    let s = inspect(&w.q());
    assert_eq!(s.state, EntryState::ForeignLink);
    assert!(!remove(&w.q()).done);
    assert!(w.mods.join("RS_Fiction").is_symlink());
}

#[test]
fn a_real_folder_that_replaced_our_link_is_never_removed() {
    let w = world();
    assert!(create(&w.q(), SYMLINK, &w.clock).done);
    std::fs::remove_file(w.mods.join("RS_Fiction")).unwrap();
    fs_err::create_dir_all(w.mods.join("RS_Fiction")).unwrap();
    fs_err::write(w.mods.join("RS_Fiction/mine.txt"), "x").unwrap();
    assert_eq!(inspect(&w.q()).state, EntryState::ForeignFolder);
    assert!(!remove(&w.q()).done);
    assert!(w.mods.join("RS_Fiction/mine.txt").is_file());
}

#[test]
fn a_running_game_blocks_create_until_confirmed_and_never_blocks_remove() {
    let w = world();
    w.probe.add(4242, "RimWorldLinux", None);
    assert_eq!(inspect(&w.q()).running, GameRunning::Running);
    let blocked = create(&w.q(), SYMLINK, &w.clock);
    assert!(!blocked.done);
    assert_eq!(blocked.refusal.unwrap().code, codes::GAME_RUNNING);
    assert!(tree(&w.mods).is_empty());
    let confirmed = create(
        &w.q(),
        CreateOptions {
            confirm_game_running: true,
            ..SYMLINK
        },
        &w.clock,
    );
    assert!(confirmed.done, "{:?}", confirmed.refusal);
    let removed = remove(&w.q());
    assert!(removed.done, "removal does not ask about the game");
    assert!(tree(&w.mods).is_empty());
}

#[test]
fn a_hidden_process_table_is_reported_as_unknown() {
    let w = world();
    let mut q = w.q();
    q.ports.hides_processes = true;
    assert_eq!(inspect(&q).running, GameRunning::Unknown);
    assert!(create(&q, SYMLINK, &w.clock).done, "unknown does not block");
}

#[test]
fn a_missing_mods_folder_gives_a_diagnostic_and_the_manual_command() {
    let w = world();
    fs_err::remove_dir(&w.mods).unwrap();
    let s = inspect(&w.q());
    assert_eq!(s.state, EntryState::Unavailable);
    assert!(!s.mods_exists);
    assert!(s.diagnostics.iter().any(|d| d.code == codes::MODS_MISSING));
    assert!(s.manual_command.contains("ln -s"));
    let out = create(&w.q(), SYMLINK, &w.clock);
    assert!(!out.done);
    assert_eq!(out.refusal.unwrap().code, codes::MODS_MISSING);
    assert!(!w.mods.exists(), "the Mods folder is not created");
}

#[test]
fn a_read_only_mods_folder_is_refused_and_leaves_no_record() {
    let w = world_with(DiskLinks {
        fail_create: true,
        ..DiskLinks::normal()
    });
    let out = create(&w.q(), SYMLINK, &w.clock);
    assert!(!out.done);
    assert_eq!(out.refusal.unwrap().code, codes::MODS_READ_ONLY);
    assert!(w.manifest.list_for(&w.mods).unwrap().is_empty());
    assert!(out.status.manual_command.starts_with("ln -s"));
}

#[test]
fn a_real_read_only_mods_folder_is_reported() {
    let w = world();
    fs_err::set_permissions(&w.mods, std::fs::Permissions::from_mode(0o555)).unwrap();
    let s = inspect(&w.q());
    let out = create(&w.q(), SYMLINK, &w.clock);
    fs_err::set_permissions(&w.mods, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(s.mods_read_only);
    if out.done {
        // Running as a user that ignores permission bits (root): nothing to assert beyond success.
        return;
    }
    assert_eq!(out.refusal.unwrap().code, codes::MODS_READ_ONLY);
    assert!(tree(&w.mods).is_empty());
    assert!(w.manifest.list_for(&w.mods).unwrap().is_empty());
}

#[test]
fn a_project_inside_the_game_folder_is_refused() {
    let w = world();
    let inside = w.install.join("Data/Core");
    let out = create(&w.query(&inside), SYMLINK, &w.clock);
    assert!(!out.done);
    assert_eq!(out.refusal.unwrap().code, codes::TARGET_REFUSED);
    assert!(tree(&w.mods).is_empty());
    assert!(w.manifest.list_for(&w.mods).unwrap().is_empty());
}

#[test]
fn a_project_folder_that_is_a_link_into_the_install_is_refused() {
    let w = world();
    let sneaky = w.base.join("MyMods/Sneaky");
    std::os::unix::fs::symlink(w.install.join("Data"), &sneaky).unwrap();
    let out = create(&w.query(&sneaky), SYMLINK, &w.clock);
    assert!(!out.done, "{:?}", out.status.state);
    assert_eq!(out.refusal.unwrap().code, codes::TARGET_REFUSED);
    assert!(tree(&w.mods).is_empty());
}

#[test]
fn a_project_folder_that_is_a_link_to_a_real_folder_links_the_real_folder() {
    let w = world();
    let alias = w.base.join("MyMods/Alias");
    std::os::unix::fs::symlink(&w.project, &alias).unwrap();
    let out = create(&w.query(&alias), SYMLINK, &w.clock);
    assert!(out.done, "{:?}", out.refusal);
    assert_eq!(out.status.name, "Alias");
    assert_eq!(out.status.points_to.as_deref(), Some(w.project.as_path()));
}

#[test]
fn the_project_folder_name_is_made_safe() {
    assert_eq!(
        link_name(Utf8Path::new("/a/RS_Fiction")).as_deref(),
        Some("RS_Fiction")
    );
    assert_eq!(
        link_name(Utf8Path::new("/a/.hidden")).as_deref(),
        Some("_.hidden")
    );
    assert_eq!(link_name(Utf8Path::new("/a/con")).as_deref(), Some("_con"));
    assert_eq!(
        link_name(Utf8Path::new("/a/my mod")).as_deref(),
        Some("my mod")
    );
    assert_eq!(link_name(Utf8Path::new("/")), None);
    assert_eq!(link_name(Utf8Path::new("/a/..")), None);
    let w = world();
    let hidden = w.base.join("MyMods/.dotted");
    fs_err::create_dir_all(&hidden).unwrap();
    let out = create(&w.query(&hidden), SYMLINK, &w.clock);
    assert!(out.done, "{:?}", out.refusal);
    assert_eq!(tree(&w.mods), ["link _.dotted"]);
}

#[test]
fn a_mode_the_system_lacks_is_refused_before_anything_is_written() {
    let w = world();
    let out = create(
        &w.q(),
        CreateOptions {
            mode: EntryMode::Junction,
            ..SYMLINK
        },
        &w.clock,
    );
    assert!(!out.done);
    assert_eq!(out.refusal.unwrap().code, codes::MODE_UNSUPPORTED);
    assert!(w.manifest.list_for(&w.mods).unwrap().is_empty());
    let w = world_with(DiskLinks {
        support: LinkSupport {
            symlink: false,
            junction: false,
            needs_privilege: true,
        },
        fail_create: false,
    });
    let out = create(&w.q(), SYMLINK, &w.clock);
    assert_eq!(out.refusal.unwrap().code, codes::MODE_UNSUPPORTED);
}

#[test]
fn a_record_without_an_entry_is_dropped_and_the_next_create_works() {
    let w = world();
    // The state a crash leaves: the record was written, the link was not made.
    assert!(create(&w.q(), SYMLINK, &w.clock).done);
    std::fs::remove_file(w.mods.join("RS_Fiction")).unwrap();
    let s = inspect(&w.q());
    assert_eq!(s.state, EntryState::NotLinked);
    assert!(s.record.is_some());
    let again = create(&w.q(), SYMLINK, &w.clock);
    assert!(again.done, "{:?}", again.refusal);
    assert_eq!(again.status.state, EntryState::Linked);
    assert_eq!(w.manifest.list_for(&w.mods).unwrap().len(), 1);
}

#[test]
fn a_damaged_record_file_is_reported_and_does_not_block_the_status() {
    let w = world();
    assert!(create(&w.q(), SYMLINK, &w.clock).done);
    for e in fs_err::read_dir(w.base.join("data/game-links")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().is_some_and(|x| x == "json") {
            fs_err::write(p, "{ not json").unwrap();
        }
    }
    let s = inspect(&w.q());
    // The link is real but unrecorded: it is not ours any more, so it is never removed.
    assert!(matches!(
        s.state,
        EntryState::LinkedByHand | EntryState::ForeignLink
    ));
    assert!(!remove(&w.q()).done);
    assert!(w.mods.join("RS_Fiction").is_symlink());
}

#[test]
fn a_marked_copy_is_a_copy_and_is_removed_only_with_its_marker() {
    let w = world();
    let out = create(
        &w.q(),
        CreateOptions {
            mode: EntryMode::Copy,
            ..SYMLINK
        },
        &w.clock,
    );
    assert!(out.done, "{:?}", out.refusal);
    assert_eq!(out.status.state, EntryState::Copy);
    assert!(w.mods.join("RS_Fiction/About/About.xml").is_file());
    assert!(w.mods.join("RS_Fiction/.rimstudio.json").is_file());
    assert!(w.project.join("About/About.xml").is_file());
    assert!(
        !w.project.join(".rimstudio.json").exists(),
        "nothing is written into the project"
    );
    let gone = remove(&w.q());
    assert!(gone.done, "{:?}", gone.refusal);
    assert!(tree(&w.mods).is_empty());
    assert!(w.project.join("About/About.xml").is_file());
}

#[test]
fn a_copy_whose_marker_was_deleted_is_foreign() {
    let w = world();
    assert!(
        create(
            &w.q(),
            CreateOptions {
                mode: EntryMode::Copy,
                ..SYMLINK
            },
            &w.clock
        )
        .done
    );
    fs_err::remove_file(w.mods.join("RS_Fiction/.rimstudio.json")).unwrap();
    assert_eq!(inspect(&w.q()).state, EntryState::ForeignFolder);
    assert!(!remove(&w.q()).done);
    assert!(w.mods.join("RS_Fiction/About/About.xml").is_file());
}

#[test]
fn a_project_folder_directly_inside_mods_is_already_visible() {
    let w = world();
    let inside = w.mods.join("RS_Direct");
    fs_err::create_dir_all(&inside).unwrap();
    let s = inspect(&w.query(&inside));
    assert_eq!(s.state, EntryState::InMods);
    assert!(!create(&w.query(&inside), SYMLINK, &w.clock).done);
}

#[test]
fn the_install_is_otherwise_untouched_by_a_full_cycle() {
    let w = world();
    let before = tree(&w.install);
    assert!(create(&w.q(), SYMLINK, &w.clock).done);
    let during = tree(&w.install);
    assert_eq!(
        during
            .iter()
            .filter(|l| !before.contains(l))
            .collect::<Vec<_>>(),
        ["link Mods/RS_Fiction"]
    );
    assert!(remove(&w.q()).done);
    assert_eq!(tree(&w.install), before);
    assert_eq!(
        fs_err::read_to_string(w.install.join("Version.txt")).unwrap(),
        "1.6.0"
    );
}

#[allow(dead_code)]
fn _clock_is_a_clock(c: &dyn Clock) -> u64 {
    c.now_unix_ms()
}
