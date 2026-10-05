//! The narrow allowance of the game write fence: a link or a copy named after a project may be created
//! directly inside `<install>/Mods` (and removed again), and nothing else under the install is written.
//! Names are fictional.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::{Clock, LinkBackend, LinkKind, LinkSupport, PortResult};
use rimstudio_io::GuardError;
use rimstudio_io::fence::{FenceConfig, FenceFs, FenceOp, GameIdle, GameWriteFence, RealFenceFs};

struct TestClock(AtomicU64);

impl Clock for TestClock {
    fn now_unix_ms(&self) -> u64 {
        self.0.fetch_add(1, Ordering::SeqCst)
    }
}

/// Real directory links on disk (Unix only; the tests that need it are gated).
#[cfg(unix)]
struct DiskLinks;

#[cfg(unix)]
impl LinkBackend for DiskLinks {
    fn support(&self) -> LinkSupport {
        LinkSupport {
            symlink: true,
            junction: false,
            needs_privilege: false,
        }
    }
    fn create_dir_link(&self, target: &Utf8Path, link: &Utf8Path) -> PortResult<LinkKind> {
        std::os::unix::fs::symlink(target, link).map_err(|e| {
            rimstudio_core::ports::PortError::new(
                rimstudio_core::ports::PortErrorKind::Io,
                e.to_string(),
            )
        })?;
        Ok(LinkKind::Symlink)
    }
    fn remove_link(&self, link: &Utf8Path) -> PortResult<()> {
        std::fs::remove_file(link).map_err(|e| {
            rimstudio_core::ports::PortError::new(
                rimstudio_core::ports::PortErrorKind::Io,
                e.to_string(),
            )
        })
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

/// A backend that must never be called: used to prove a refusal happens before any link is made.
struct NeverLinks;

impl LinkBackend for NeverLinks {
    fn support(&self) -> LinkSupport {
        LinkSupport {
            symlink: true,
            junction: false,
            needs_privilege: false,
        }
    }
    fn create_dir_link(&self, _: &Utf8Path, _: &Utf8Path) -> PortResult<LinkKind> {
        panic!("a refused operation must not reach the link backend");
    }
    fn remove_link(&self, _: &Utf8Path) -> PortResult<()> {
        panic!("a refused operation must not reach the link backend");
    }
    fn is_link(&self, _: &Utf8Path) -> bool {
        false
    }
    fn read_link(&self, _: &Utf8Path) -> Option<Utf8PathBuf> {
        None
    }
}

struct Fixture {
    _t: tempfile::TempDir,
    install: Utf8PathBuf,
    config: Utf8PathBuf,
    mods: Utf8PathBuf,
    project: Utf8PathBuf,
    fence: GameWriteFence,
}

fn fixture(links: Arc<dyn LinkBackend>) -> Fixture {
    let t = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(dunce::canonicalize(t.path()).unwrap()).unwrap();
    let install = base.join("RimWorld");
    let config = base.join("GameConfig");
    let mods = install.join("Mods");
    let project = base.join("MyMods/RS_Fiction");
    for d in [&mods, &config, &project, &install.join("Data/Core")] {
        fs_err::create_dir_all(d).unwrap();
    }
    fs_err::write(project.join("About.txt"), "a").unwrap();
    fs_err::write(install.join("Version.txt"), "1.6.0 rev1").unwrap();
    fs_err::write(install.join("Data/Core/x.xml"), "core").unwrap();
    fs_err::write(config.join("ModsConfig.xml"), "old").unwrap();
    let fence = GameWriteFence::new(
        FenceConfig {
            install_mods: mods.clone(),
            mods_config: config.join("ModsConfig.xml"),
            protected: vec![install.clone(), config.clone()],
            backup: FenceConfig::mods_config_backup_policy(base.join("backups")),
        },
        Arc::new(RealFenceFs),
        links,
        Arc::new(TestClock(AtomicU64::new(1_894_708_800_000))),
    )
    .unwrap();
    Fixture {
        _t: t,
        install,
        config,
        mods,
        project,
        fence,
    }
}

/// Every file and folder below `root` with its kind, sorted, links not followed.
fn snapshot(root: &Utf8Path) -> Vec<String> {
    fn walk(dir: &Utf8Path, out: &mut Vec<String>, root: &Utf8Path) {
        let mut entries: Vec<_> = fs_err::read_dir(dir).unwrap().map(Result::unwrap).collect();
        entries.sort_by_key(fs_err::DirEntry::file_name);
        for e in entries {
            let p = Utf8PathBuf::from_path_buf(e.path()).unwrap();
            let ft = e.file_type().unwrap();
            let rel = p.strip_prefix(root).unwrap().to_string();
            if ft.is_symlink() {
                out.push(format!("link {rel}"));
            } else if ft.is_dir() {
                out.push(format!("dir {rel}"));
                walk(&p, out, root);
            } else {
                out.push(format!(
                    "file {rel} {}",
                    fs_err::read_to_string(&p).unwrap()
                ));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, &mut out, root);
    out
}

fn is_refusal(r: &Result<(), GuardError>) -> bool {
    matches!(r, Err(GuardError::FenceRefused { .. }))
}

#[cfg(unix)]
#[test]
fn a_link_named_after_the_project_is_created_and_removed_and_nothing_else_changes() {
    let f = fixture(Arc::new(DiskLinks));
    let before = snapshot(&f.install);
    f.fence
        .apply(&FenceOp::CreateOwnedLink {
            name: "RS_Fiction",
            target: &f.project,
        })
        .unwrap();
    let after = snapshot(&f.install);
    let added: Vec<_> = after.iter().filter(|l| !before.contains(l)).collect();
    assert_eq!(added, ["link Mods/RS_Fiction"], "exactly one link is new");
    assert!(
        before.iter().all(|l| after.contains(l)),
        "nothing else changed"
    );
    f.fence
        .apply(&FenceOp::RemoveOwnedLink { name: "RS_Fiction" })
        .unwrap();
    assert_eq!(snapshot(&f.install), before);
    assert!(f.project.join("About.txt").is_file());
}

#[test]
fn a_target_inside_the_game_folder_is_refused() {
    let f = fixture(Arc::new(NeverLinks));
    for target in [
        f.install.join("Data"),
        f.install.join("Data/Core"),
        f.install.clone(),
        f.mods.clone(),
        f.install.parent().unwrap().to_owned(),
    ] {
        let r = f.fence.apply(&FenceOp::CreateOwnedLink {
            name: "RS_Fiction",
            target: &target,
        });
        assert!(is_refusal(&r), "{target}: {r:?}");
        let r = f.fence.apply(&FenceOp::CreateOwnedCopy {
            name: "RS_Fiction",
            source: &target,
        });
        assert!(is_refusal(&r), "{target}: {r:?}");
    }
    assert!(f.fence.performed().is_empty());
}

#[test]
fn a_name_with_a_traversal_or_a_separator_cannot_leave_the_mods_folder() {
    let f = fixture(Arc::new(NeverLinks));
    for name in [
        "..",
        "../Data",
        "../Evil",
        "..\\Evil",
        "a/b",
        "/etc",
        "C:\\x",
        ".hidden",
        "",
        "Mods/../Data",
        "x\0y",
    ] {
        let r = f.fence.apply(&FenceOp::CreateOwnedLink {
            name,
            target: &f.project,
        });
        assert!(is_refusal(&r), "{name:?}: {r:?}");
        let r = f.fence.apply(&FenceOp::RemoveOwnedLink { name });
        assert!(is_refusal(&r), "{name:?}: {r:?}");
    }
    assert!(f.fence.performed().is_empty());
}

#[test]
fn there_is_no_way_to_remove_or_replace_what_is_in_the_install() {
    let f = fixture(Arc::new(NeverLinks));
    fs_err::create_dir_all(f.mods.join("Foreign")).unwrap();
    let before = snapshot(&f.install);
    for name in ["Foreign", "Data", "Version.txt", "Core"] {
        assert!(is_refusal(
            &f.fence.apply(&FenceOp::RemoveOwnedLink { name })
        ));
        assert!(is_refusal(
            &f.fence.apply(&FenceOp::RemoveOwnedCopy { name })
        ));
    }
    let r = f.fence.apply(&FenceOp::CreateOwnedLink {
        name: "Foreign",
        target: &f.project,
    });
    assert!(is_refusal(&r), "an existing folder is never replaced");
    assert_eq!(snapshot(&f.install), before);
}

#[test]
fn project_writes_inside_the_install_or_config_folder_are_refused() {
    let f = fixture(Arc::new(NeverLinks));
    let guard = rimstudio_io::RootGuard::new([&f.install, &f.config, &f.project]).unwrap();
    for path in [
        f.install.join("Data/Core/x.xml"),
        f.install.join("Version.txt"),
        f.install.join("Mods/RS_Fiction/About.xml"),
        f.install.join("NewFile.txt"),
    ] {
        assert!(
            f.fence.check_project_write(&guard, &path).is_err(),
            "{path}"
        );
    }
    assert!(
        f.fence
            .check_project_write(&guard, &f.project.join("Defs/a.xml"))
            .is_ok()
    );
}

#[test]
fn the_only_other_write_is_the_mods_config_file_and_it_is_backed_up() {
    let f = fixture(Arc::new(NeverLinks));
    let before = snapshot(&f.install);
    f.fence
        .apply(&FenceOp::WriteModsConfig {
            bytes: b"new",
            game: GameIdle::NotRunning,
        })
        .unwrap();
    assert_eq!(snapshot(&f.install), before, "the install is untouched");
    assert_eq!(
        fs_err::read_to_string(f.config.join("ModsConfig.xml")).unwrap(),
        "new"
    );
    let paths: Vec<_> = f.fence.performed().into_iter().map(|r| r.path).collect();
    assert_eq!(paths, [f.config.join("ModsConfig.xml")]);
}

#[allow(dead_code)]
fn _fence_fs_is_object_safe(_: &dyn FenceFs) {}
