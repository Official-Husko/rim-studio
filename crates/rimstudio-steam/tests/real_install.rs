//! Tests that read a real machine. They are ignored by default and only read: nothing is written
//! and no output is stored. Set the variables to run them:
//!
//! - `RIMSTUDIO_GAME_DIR`: the RimWorld install folder;
//! - `RIMSTUDIO_WORKSHOP_DIR`: the Steam workshop content folder for app 294100.
//!
//! The differential test against `steamlocate` needs a Steam installation of the current user.

use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::os::Os;
use rimstudio_core::ports::{
    Clock, DetectEnv, DetectEnvRef, DirEntryInfo, EntryKind, EnvProbe, FsMeta, FsProbe, Hive,
    PortError, PortErrorKind, PortResult, RegistryProbe,
};
use rimstudio_steam::locator::{DetectOptions, detect};
use rimstudio_steam::report::InstallKind;
use rimstudio_steam::workshop::{ItemState, status_from_env};

struct RealClock;
impl Clock for RealClock {
    fn now_unix_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(0))
    }
}

struct RealEnv;
impl EnvProbe for RealEnv {
    fn var(&self, key: &str) -> Option<String> {
        std::env::var(key).ok()
    }
    fn home_dir(&self) -> Option<Utf8PathBuf> {
        std::env::var("HOME").ok().map(Utf8PathBuf::from)
    }
    fn exe_dir(&self) -> Option<Utf8PathBuf> {
        None
    }
}

struct NoRegistry;
impl RegistryProbe for NoRegistry {
    fn read_string(&self, _hive: Hive, _key: &str, _value: &str) -> Option<String> {
        None
    }
}

/// A read only file system probe over `std::fs` for this test file.
struct RealFs;

fn io_error(e: &std::io::Error) -> PortError {
    let kind = match e.kind() {
        std::io::ErrorKind::NotFound => PortErrorKind::NotFound,
        std::io::ErrorKind::PermissionDenied => PortErrorKind::PermissionDenied,
        _ => PortErrorKind::Io,
    };
    PortError::new(kind, e.to_string())
}

fn kind_of(ft: std::fs::FileType) -> EntryKind {
    if ft.is_symlink() {
        EntryKind::Symlink
    } else if ft.is_dir() {
        EntryKind::Dir
    } else if ft.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    }
}

impl FsProbe for RealFs {
    fn exists(&self, path: &Utf8Path, _d: Duration) -> PortResult<bool> {
        Ok(path.as_std_path().exists())
    }
    fn is_dir(&self, path: &Utf8Path, _d: Duration) -> PortResult<bool> {
        Ok(path.as_std_path().is_dir())
    }
    fn metadata(&self, path: &Utf8Path, _d: Duration) -> PortResult<FsMeta> {
        let m = std::fs::symlink_metadata(path.as_std_path()).map_err(|e| io_error(&e))?;
        Ok(FsMeta {
            kind: kind_of(m.file_type()),
            size: m.len(),
            mtime_ns: m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .and_then(|d| i128::try_from(d.as_nanos()).ok()),
            file_id: None,
        })
    }
    fn read_dir(&self, path: &Utf8Path, _d: Duration) -> PortResult<Vec<DirEntryInfo>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(path.as_std_path()).map_err(|e| io_error(&e))? {
            let entry = entry.map_err(|e| io_error(&e))?;
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let Ok(ft) = entry.file_type() else { continue };
            // Like the port contract: links are reported as links.
            out.push(DirEntryInfo {
                name,
                kind: kind_of(ft),
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }
    fn read_to_string(&self, path: &Utf8Path, _d: Duration) -> PortResult<String> {
        std::fs::read_to_string(path.as_std_path()).map_err(|e| io_error(&e))
    }
    fn canonicalize(&self, path: &Utf8Path, _d: Duration) -> PortResult<Utf8PathBuf> {
        let p = std::fs::canonicalize(path.as_std_path()).map_err(|e| io_error(&e))?;
        Utf8PathBuf::from_path_buf(p)
            .map_err(|_| PortError::new(PortErrorKind::Io, "path is not UTF-8"))
    }
}

fn with_env<T>(f: impl FnOnce(&dyn DetectEnv) -> T) -> T {
    let env = DetectEnvRef {
        clock: &RealClock,
        env: &RealEnv,
        registry: &NoRegistry,
        fs: &RealFs,
    };
    f(&env)
}

fn canonical(path: &str) -> Utf8PathBuf {
    let p = std::fs::canonicalize(path)
        .unwrap_or_else(|e| panic!("the folder from the environment variable must exist: {e}"));
    Utf8PathBuf::from_path_buf(p).unwrap_or_else(|p| panic!("the path is not UTF-8: {p:?}"))
}

#[test]
#[ignore = "reads the real machine; set RIMSTUDIO_GAME_DIR"]
fn detection_finds_the_install_named_by_the_environment() {
    let Ok(game_dir) = std::env::var("RIMSTUDIO_GAME_DIR") else {
        println!("RIMSTUDIO_GAME_DIR is not set; nothing to check");
        return;
    };
    let wanted = canonical(&game_dir);
    let report = with_env(|env| detect(env, &DetectOptions::new(Os::current())));
    let found = report
        .installs
        .iter()
        .find(|i| {
            std::fs::canonicalize(i.path.as_std_path()).ok().as_deref()
                == Some(wanted.as_std_path())
        })
        .unwrap_or_else(|| {
            panic!(
                "no install at the game folder; installs: {}",
                report.installs.len()
            )
        });
    assert!(found.version.is_some(), "Version.txt should parse");
    assert!(found.mods_dir.exists);
    assert!(found.data_dirs.iter().any(|d| d == "Core"));
    if found.kind == InstallKind::Steam {
        assert!(!report.libraries.is_empty());
    }
}

#[test]
#[ignore = "reads the real machine; set RIMSTUDIO_WORKSHOP_DIR"]
fn workshop_status_reads_the_real_workshop_manifest() {
    let Ok(dir) = std::env::var("RIMSTUDIO_WORKSHOP_DIR") else {
        println!("RIMSTUDIO_WORKSHOP_DIR is not set; nothing to check");
        return;
    };
    let content = Utf8PathBuf::from(dir);
    let acf = content
        .parent()
        .and_then(Utf8Path::parent)
        .map(|w| w.join("appworkshop_294100.acf"))
        .expect("the content folder has a workshop parent");
    let statuses = with_env(|env| status_from_env(env, &content, &acf, Duration::from_secs(5)))
        .expect("the manifest parses");
    assert!(!statuses.is_empty());
    let count = |s: ItemState| statuses.iter().filter(|x| x.state == s).count();
    println!(
        "items {} current {} update {} downloading {} missing {} orphan {}",
        statuses.len(),
        count(ItemState::Current),
        count(ItemState::UpdateAvailable),
        count(ItemState::Downloading),
        count(ItemState::NotDownloaded),
        count(ItemState::Orphan),
    );
    assert!(statuses.windows(2).all(|w| w[0].id < w[1].id));
}

#[test]
#[ignore = "needs a real Steam installation of the current user"]
fn detection_agrees_with_steamlocate() {
    let Ok(steam) = steamlocate::SteamDir::locate() else {
        println!("steamlocate found no Steam; nothing to compare");
        return;
    };
    let report = with_env(|env| detect(env, &DetectOptions::new(Os::current())));
    let canon = |p: &Utf8Path| std::fs::canonicalize(p.as_std_path()).ok();

    let their_root = std::fs::canonicalize(steam.path()).expect("their root exists");
    assert!(
        report
            .steam_roots
            .iter()
            .any(|r| canon(&r.canonical).as_deref() == Some(their_root.as_path())),
        "our roots miss steamlocate's root"
    );

    let their_libs: Vec<_> = steam
        .libraries()
        .expect("libraries listed")
        .filter_map(Result::ok)
        .filter_map(|l| std::fs::canonicalize(l.path()).ok())
        .collect();
    for lib in &their_libs {
        assert!(
            report
                .libraries
                .iter()
                .any(|l| canon(&l.canonical).as_deref() == Some(lib.as_path())),
            "our libraries miss {lib:?}"
        );
    }

    if let Ok(Some((app, library))) = steam.find_app(294_100) {
        let their_install = std::fs::canonicalize(library.resolve_app_dir(&app)).ok();
        assert!(
            report
                .installs
                .iter()
                .any(|i| canon(&i.path) == their_install),
            "our installs miss steamlocate's install"
        );
    }
}
