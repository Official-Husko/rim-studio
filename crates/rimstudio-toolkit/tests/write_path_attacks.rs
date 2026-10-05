//! Attacks on the guarded writer: every hostile path must be refused with a clear error and nothing may
//! be written, whatever the plan or the folder looks like. Names are fictional; the tests write only into
//! temporary folders.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::collections::BTreeSet;
use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_testing::fakes::FakeClock;
use rimstudio_toolkit::error::ToolkitError;
use rimstudio_toolkit::shared::writer::GuardedWriter;

struct Sandbox {
    _tmp: tempfile::TempDir,
    base: Utf8PathBuf,
    root: Utf8PathBuf,
}

fn sandbox() -> Sandbox {
    let tmp = tempfile::tempdir().unwrap();
    let base = Utf8PathBuf::from_path_buf(tmp.path().canonicalize().unwrap()).unwrap();
    let root = base.join("proj");
    std::fs::create_dir_all(root.join("Defs").as_std_path()).unwrap();
    std::fs::create_dir_all(base.join("outside").as_std_path()).unwrap();
    Sandbox {
        _tmp: tmp,
        base,
        root,
    }
}

fn writer_with(s: &Sandbox, root: &Utf8Path, protected: &[Utf8PathBuf]) -> GuardedWriter {
    GuardedWriter::new(
        root,
        protected,
        s.base.join("backups"),
        None,
        Arc::new(FakeClock::new(FakeClock::DEFAULT_START_MS)),
    )
    .unwrap()
}

fn writer(s: &Sandbox) -> GuardedWriter {
    writer_with(s, &s.root.clone(), &[])
}

/// Every file and folder below `dir`, as relative text, sorted.
fn snapshot(dir: &Utf8Path) -> BTreeSet<String> {
    fn walk(dir: &Utf8Path, base: &Utf8Path, out: &mut BTreeSet<String>) {
        let Ok(rd) = std::fs::read_dir(dir.as_std_path()) else {
            return;
        };
        for e in rd.flatten() {
            let p = Utf8PathBuf::from_path_buf(e.path()).unwrap();
            out.insert(p.strip_prefix(base).unwrap().to_string());
            let is_real_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_real_dir {
                walk(&p, base, out);
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(dir, dir, &mut out);
    out
}

fn assert_refused(r: Result<impl std::fmt::Debug, ToolkitError>, what: &str) -> ToolkitError {
    match r {
        Err(e @ ToolkitError::PathRefused { .. }) => e,
        other => panic!("{what}: expected PathRefused, got {other:?}"),
    }
}

#[test]
fn hostile_relative_paths_are_refused_and_nothing_is_written() {
    let s = sandbox();
    let w = writer(&s);
    let long_name = format!("Defs/{}.xml", "a".repeat(300));
    let long_path = format!("{}/x.xml", vec!["segment"; 60].join("/"));
    let cases: Vec<(&str, String)> = vec![
        ("absolute", s.base.join("outside/x.xml").to_string()),
        ("absolute unix", "/etc/rs_x.xml".to_owned()),
        ("parent", "../outside/x.xml".to_owned()),
        (
            "parent in the middle",
            "Defs/../../outside/x.xml".to_owned(),
        ),
        ("drive", "C:/Users/x.xml".to_owned()),
        ("drive relative", "C:x.xml".to_owned()),
        ("unc", "//server/share/x.xml".to_owned()),
        ("unc backslash", "\\\\server\\share\\x.xml".to_owned()),
        ("verbatim", "\\\\?\\C:\\x.xml".to_owned()),
        ("backslash", "Defs\\x.xml".to_owned()),
        ("trailing dot", "Defs/x.xml.".to_owned()),
        ("trailing space", "Defs/x.xml ".to_owned()),
        ("trailing dot folder", "Defs./x.xml".to_owned()),
        ("device con", "Defs/CON".to_owned()),
        ("device con ext", "Defs/con.xml".to_owned()),
        ("device nul", "NUL".to_owned()),
        ("device com1", "Defs/COM1.xml".to_owned()),
        ("device lpt9 folder", "lpt9/x.xml".to_owned()),
        ("long name", long_name),
        ("long path", long_path),
        ("nul byte", "Defs/a\0b.xml".to_owned()),
        ("control", "Defs/a\u{7}b.xml".to_owned()),
        ("stream", "Defs/x.xml:evil".to_owned()),
        ("wildcard", "Defs/*.xml".to_owned()),
        ("empty", String::new()),
        ("dot", "./x.xml".to_owned()),
        ("empty segment", "Defs//x.xml".to_owned()),
        ("trailing slash", "Defs/".to_owned()),
    ];
    let before = snapshot(&s.base);
    for (what, path) in &cases {
        assert_refused(w.write(path, "x", false), what);
        assert_refused(w.write(path, "x", true), what);
        assert_refused(w.make_dir(path), what);
        assert_refused(w.path_of(path), what);
    }
    assert_eq!(snapshot(&s.base), before, "nothing was created anywhere");
}

#[test]
fn a_refusal_names_the_reason() {
    let s = sandbox();
    let w = writer(&s);
    let text = assert_refused(w.write("Defs/CON.xml", "x", false), "device").to_string();
    assert!(text.contains("reserved"), "{text}");
    let text = assert_refused(w.write("Defs/x.xml.", "x", false), "dot").to_string();
    assert!(text.contains("trailing"), "{text}");
}

#[cfg(unix)]
#[test]
fn a_directory_link_that_leaves_the_project_is_refused_and_no_folder_is_created_behind_it() {
    let s = sandbox();
    std::os::unix::fs::symlink(
        s.base.join("outside").as_std_path(),
        s.root.join("Textures").as_std_path(),
    )
    .unwrap();
    let w = writer(&s);
    let before = snapshot(&s.base.join("outside"));
    assert_refused(w.write("Textures/x.xml", "x", false), "file");
    assert_refused(w.write("Textures/new/deeper/x.xml", "x", true), "deep file");
    assert_refused(w.make_dir("Textures/made"), "folder");
    assert_eq!(snapshot(&s.base.join("outside")), before);
}

#[cfg(unix)]
#[test]
fn a_file_link_that_leaves_the_project_is_refused_and_the_target_stays_intact() {
    let s = sandbox();
    let target = s.base.join("outside/victim.xml");
    std::fs::write(target.as_std_path(), "precious").unwrap();
    std::os::unix::fs::symlink(
        target.as_std_path(),
        s.root.join("Defs/victim.xml").as_std_path(),
    )
    .unwrap();
    let w = writer(&s);
    assert_refused(w.write("Defs/victim.xml", "changed", true), "file link");
    assert_eq!(
        std::fs::read_to_string(target.as_std_path()).unwrap(),
        "precious"
    );
}

#[cfg(unix)]
#[test]
fn a_dangling_link_in_the_project_is_refused() {
    let s = sandbox();
    std::os::unix::fs::symlink(
        s.base.join("outside/not-there.xml").as_std_path(),
        s.root.join("Defs/dangling.xml").as_std_path(),
    )
    .unwrap();
    let w = writer(&s);
    assert_refused(w.write("Defs/dangling.xml", "x", false), "dangling");
    assert!(!s.base.join("outside/not-there.xml").exists());
}

#[cfg(unix)]
#[test]
fn a_file_link_that_stays_inside_the_project_is_not_replaced_by_a_plain_file() {
    let s = sandbox();
    std::fs::write(s.root.join("Defs/real.xml").as_std_path(), "real").unwrap();
    std::os::unix::fs::symlink(
        s.root.join("Defs/real.xml").as_std_path(),
        s.root.join("Defs/alias.xml").as_std_path(),
    )
    .unwrap();
    let w = writer(&s);
    assert_refused(
        w.write("Defs/alias.xml", "changed", true),
        "inner file link",
    );
    let meta = std::fs::symlink_metadata(s.root.join("Defs/alias.xml").as_std_path()).unwrap();
    assert!(meta.file_type().is_symlink(), "the link is still a link");
    assert_eq!(
        std::fs::read_to_string(s.root.join("Defs/real.xml").as_std_path()).unwrap(),
        "real"
    );
}

#[cfg(unix)]
#[test]
fn a_project_folder_that_is_itself_a_link_is_refused() {
    let s = sandbox();
    let link = s.base.join("proj-link");
    std::os::unix::fs::symlink(s.root.as_std_path(), link.as_std_path()).unwrap();
    let r = GuardedWriter::new(
        &link,
        &[],
        s.base.join("backups"),
        None,
        Arc::new(FakeClock::new(1)),
    );
    let text = assert_refused(r, "root link").to_string();
    assert!(text.contains("link"), "{text}");
}

#[cfg(unix)]
#[test]
fn a_project_folder_reached_through_a_linked_parent_is_written_inside_the_real_folder() {
    let s = sandbox();
    let parent_link = s.base.join("parent-link");
    std::os::unix::fs::symlink(s.base.as_std_path(), parent_link.as_std_path()).unwrap();
    let w = writer_with(&s, &parent_link.join("proj"), &[]);
    w.write("Defs/ok.xml", "ok", false).unwrap();
    assert_eq!(
        std::fs::read_to_string(s.root.join("Defs/ok.xml").as_std_path()).unwrap(),
        "ok"
    );
}

#[test]
fn a_project_inside_a_steam_library_is_refused_even_when_nothing_lists_it_as_protected() {
    let s = sandbox();
    for tail in [
        "steamapps/common/RS_Game/Mods/RS_Mod",
        "SteamApps/workshop/content/294100/1234",
    ] {
        let root = s.base.join("Steam").join(tail);
        std::fs::create_dir_all(root.as_std_path()).unwrap();
        let r = GuardedWriter::new(
            &root,
            &[],
            s.base.join("backups"),
            None,
            Arc::new(FakeClock::new(1)),
        );
        let text = assert_refused(r, tail).to_string();
        assert!(text.contains("Steam"), "{text}");
    }
}

#[test]
fn a_protected_folder_inside_the_project_is_refused_for_every_spelling_of_the_path() {
    let s = sandbox();
    std::fs::create_dir_all(s.root.join("Locked/inner").as_std_path()).unwrap();
    let w = writer_with(&s, &s.root.clone(), &[s.root.join("Locked")]);
    assert_refused(w.write("Locked/x.xml", "x", false), "direct");
    assert_refused(w.write("Locked/inner/x.xml", "x", false), "nested");
    assert_refused(w.write("Locked/new/x.xml", "x", false), "missing");
    assert!(w.write("Defs/x.xml", "x", false).is_ok());
}

#[test]
fn a_name_that_differs_only_by_case_from_an_existing_entry_is_refused() {
    let s = sandbox();
    std::fs::write(s.root.join("Defs/Rifle.xml").as_std_path(), "old").unwrap();
    let w = writer(&s);
    let before = snapshot(&s.base);
    let e = assert_refused(w.write("defs/Rifle.xml", "x", true), "folder case").to_string();
    assert!(e.contains("case"), "{e}");
    assert_refused(w.write("Defs/rifle.xml", "x", true), "file case");
    assert_refused(w.write("DEFS/new/x.xml", "x", true), "folder case deeper");
    assert_eq!(snapshot(&s.base), before);
    assert_eq!(
        std::fs::read_to_string(s.root.join("Defs/Rifle.xml").as_std_path()).unwrap(),
        "old"
    );
    // the exact spelling still works
    w.write("Defs/Rifle.xml", "new", true).unwrap();
}

#[cfg(unix)]
#[test]
fn a_read_only_file_is_not_replaced() {
    use std::os::unix::fs::PermissionsExt;
    let s = sandbox();
    let file = s.root.join("Defs/locked.xml");
    std::fs::write(file.as_std_path(), "keep").unwrap();
    std::fs::set_permissions(file.as_std_path(), std::fs::Permissions::from_mode(0o444)).unwrap();
    let w = writer(&s);
    let e = assert_refused(w.write("Defs/locked.xml", "x", true), "read only").to_string();
    assert!(e.contains("read only"), "{e}");
    assert_eq!(std::fs::read_to_string(file.as_std_path()).unwrap(), "keep");
    assert!(
        !s.base.join("backups").exists(),
        "no backup for a refused write"
    );
}

#[test]
fn an_accepted_write_leaves_no_temporary_or_backup_file_in_the_project() {
    let s = sandbox();
    let w = writer(&s);
    w.write("Defs/a.xml", "one", true).unwrap();
    w.write("Defs/a.xml", "two", true).unwrap();
    w.write("Defs/a.xml", "three", true).unwrap();
    let names = snapshot(&s.root);
    assert_eq!(
        names,
        BTreeSet::from(["Defs".to_owned(), "Defs/a.xml".to_owned()]),
        "{names:?}"
    );
}

#[test]
fn a_backup_that_cannot_be_made_stops_the_write_and_the_old_file_survives() {
    let s = sandbox();
    let w = writer(&s);
    w.write("Defs/a.xml", "old", false).unwrap();
    // a file where the backup folder must be makes every backup fail
    std::fs::write(s.base.join("backups").as_std_path(), "not a folder").unwrap();
    let r = w.write("Defs/a.xml", "new", true);
    assert!(r.is_err(), "{r:?}");
    assert_eq!(
        std::fs::read_to_string(s.root.join("Defs/a.xml").as_std_path()).unwrap(),
        "old"
    );
}

#[test]
fn the_backup_of_a_replaced_file_holds_the_old_bytes_and_is_outside_the_project() {
    let s = sandbox();
    let w = writer(&s);
    w.write("Defs/a.xml", "old\r\n", false).unwrap();
    let r = w.write("Defs/a.xml", "new", true).unwrap();
    let backup = r.backup.expect("a backup");
    assert!(!backup.starts_with(&s.root), "{backup}");
    assert_eq!(std::fs::read(backup.as_std_path()).unwrap(), b"old\r\n");
}

#[test]
fn the_root_guard_the_fence_and_the_writer_agree_on_every_hostile_path() {
    use rimstudio_io::fence::{FenceConfig, GameWriteFence, RealFenceFs};
    use rimstudio_io::guard::RootGuard;
    use rimstudio_testing::fakes::FakeLinkBackend;

    let s = sandbox();
    let game = s.base.join("RS_Game");
    let config = s.base.join("RS_Config");
    for d in [game.join("Mods"), config.clone(), s.root.join("Locked")] {
        std::fs::create_dir_all(d.as_std_path()).unwrap();
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        s.base.join("outside").as_std_path(),
        s.root.join("Textures").as_std_path(),
    )
    .unwrap();
    let fence = Arc::new(
        GameWriteFence::new(
            FenceConfig {
                install_mods: game.join("Mods"),
                mods_config: config.join("ModsConfig.xml"),
                protected: vec![game.clone(), config.clone(), s.root.join("Locked")],
                backup: FenceConfig::mods_config_backup_policy(s.base.join("fence-backups")),
            },
            Arc::new(RealFenceFs),
            Arc::new(FakeLinkBackend::new()),
            Arc::new(FakeClock::new(1)),
        )
        .unwrap(),
    );
    let guard = RootGuard::new([s.root.clone()]).unwrap();
    let w = GuardedWriter::new(
        &s.root,
        &[],
        s.base.join("backups"),
        Some(fence.clone()),
        Arc::new(FakeClock::new(1)),
    )
    .unwrap();
    let mut hostile: Vec<String> = vec![
        "../outside/x.xml".into(),
        "Defs/../../outside/x.xml".into(),
        "Locked/x.xml".into(),
        "Locked/deeper/x.xml".into(),
    ];
    if cfg!(unix) {
        hostile.push("Textures/x.xml".into());
        hostile.push("Textures/new/x.xml".into());
    }
    for rel in &hostile {
        let abs = s.root.join(rel);
        assert!(w.write(rel, "x", false).is_err(), "writer accepted {rel}");
        assert!(w.path_of(rel).is_err(), "writer accepted {rel}");
        // the guard and the fence see the same paths as errors (the lexical `..` forms and links by the
        // guard, the protected folder by the fence)
        let by_guard = guard.validate(&abs).is_err();
        let by_fence = fence.check_project_write(&guard, &abs).is_err();
        assert!(
            by_guard || by_fence,
            "neither the guard nor the fence refused {rel}"
        );
        assert!(by_fence, "the fence must refuse {rel} too");
    }
    // a path every layer allows is allowed by all of them
    assert!(guard.validate(&s.root.join("Defs/ok.xml")).is_ok());
    assert!(
        fence
            .check_project_write(&guard, &s.root.join("Defs/ok.xml"))
            .is_ok()
    );
    w.write("Defs/ok.xml", "ok", false).unwrap();
    // a project root inside a protected folder is refused by the writer and by the fence
    let inside = game.join("Mods/RS_Mod");
    std::fs::create_dir_all(inside.as_std_path()).unwrap();
    let inside_guard = RootGuard::new([inside.clone()]).unwrap();
    assert!(
        fence
            .check_project_write(&inside_guard, &inside.join("Defs/x.xml"))
            .is_err()
    );
    let r = GuardedWriter::new(
        &inside,
        std::slice::from_ref(&game),
        s.base.join("backups"),
        Some(fence),
        Arc::new(FakeClock::new(1)),
    );
    assert!(matches!(r, Err(ToolkitError::PathRefused { .. })));
}
