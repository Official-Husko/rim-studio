//! The four data roots of the app and portable mode.
//!
//! [`DataRoots`] is resolved once from the standard folders the shell computes
//! ([`PlatformDirs`], a plain data type that lives in `rimstudio-core::ports` and is filled by
//! `rimstudio-platform`), the environment and a probe for the marker file `rimstudio.portable`.
//! When the marker sits beside the executable (beside the `.AppImage` file when the `APPIMAGE`
//! variable is set) all four roots move under `./data/{config,data,cache,logs}` next to it.
//! Resolution is pure given its inputs, so it is fully testable with fakes.

use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::{EnvProbe, FsProbe, PlatformDirs};

use crate::error::StoreError;

/// The name of the marker file that switches on portable mode.
pub const PORTABLE_MARKER: &str = "rimstudio.portable";

/// Which of the roots a path lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RootKind {
    /// Settings and workspace overrides (JSONC, user edited).
    Config,
    /// Durable app data (documents, drafts).
    Data,
    /// Rebuildable caches.
    Cache,
    /// Log files.
    Logs,
}

impl RootKind {
    /// All kinds in a fixed order.
    pub const ALL: [RootKind; 4] = [
        RootKind::Config,
        RootKind::Data,
        RootKind::Cache,
        RootKind::Logs,
    ];

    /// The lowercase name used in diagnostics and portable folder names.
    pub fn as_str(self) -> &'static str {
        match self {
            RootKind::Config => "config",
            RootKind::Data => "data",
            RootKind::Cache => "cache",
            RootKind::Logs => "logs",
        }
    }
}

/// The resolved root folders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataRoots {
    /// Settings and workspace overrides.
    pub config: Utf8PathBuf,
    /// Durable app data.
    pub data: Utf8PathBuf,
    /// Rebuildable caches.
    pub cache: Utf8PathBuf,
    /// Log files.
    pub logs: Utf8PathBuf,
    /// True when the portable marker moved the roots beside the executable.
    pub portable: bool,
}

impl DataRoots {
    /// Resolves the roots, probing the marker through `fs` (a two second deadline per probe).
    pub fn resolve(dirs: &PlatformDirs, env: &dyn EnvProbe, fs: &dyn FsProbe) -> DataRoots {
        Self::resolve_with(dirs, env, |p| {
            fs.exists(p, Duration::from_secs(2)).unwrap_or(false)
        })
    }

    /// Resolves the roots with a closure that says whether a marker path exists.
    pub fn resolve_with(
        dirs: &PlatformDirs,
        env: &dyn EnvProbe,
        marker_exists: impl Fn(&Utf8Path) -> bool,
    ) -> DataRoots {
        match Self::portable_base(env) {
            Some(base) if marker_exists(&base.join(PORTABLE_MARKER)) => {
                Self::under_base(&base.join("data"))
            }
            _ => Self::from_dirs(dirs),
        }
    }

    /// The folder searched for the marker: the parent of `$APPIMAGE` when set, else the folder of
    /// the executable.
    pub fn portable_base(env: &dyn EnvProbe) -> Option<Utf8PathBuf> {
        if let Some(image) = env.var("APPIMAGE").filter(|v| !v.is_empty()) {
            let parent = Utf8Path::new(&image).parent().map(Utf8Path::to_path_buf);
            if parent.as_ref().is_some_and(|p| !p.as_str().is_empty()) {
                return parent;
            }
        }
        env.exe_dir()
    }

    /// Roots equal to the standard folders (not portable).
    pub fn from_dirs(dirs: &PlatformDirs) -> DataRoots {
        DataRoots {
            config: dirs.config.clone(),
            data: dirs.data.clone(),
            cache: dirs.cache.clone(),
            logs: dirs.logs.clone(),
            portable: false,
        }
    }

    /// Roots as `base/{config,data,cache,logs}`, marked portable. Tests use it with a temp folder.
    pub fn under_base(base: &Utf8Path) -> DataRoots {
        DataRoots {
            config: base.join("config"),
            data: base.join("data"),
            cache: base.join("cache"),
            logs: base.join("logs"),
            portable: true,
        }
    }

    /// The folder of one root.
    pub fn path(&self, kind: RootKind) -> &Utf8Path {
        match kind {
            RootKind::Config => &self.config,
            RootKind::Data => &self.data,
            RootKind::Cache => &self.cache,
            RootKind::Logs => &self.logs,
        }
    }

    /// The root that contains `path` by component comparison, `None` when none does.
    pub fn kind_of(&self, path: &Utf8Path) -> Option<RootKind> {
        RootKind::ALL
            .into_iter()
            .filter(|k| path.starts_with(self.path(*k)))
            .max_by_key(|k| self.path(*k).components().count())
    }

    /// Creates every root folder.
    pub fn ensure_all(&self) -> Result<(), StoreError> {
        for kind in RootKind::ALL {
            let p = self.path(kind);
            fs_err::create_dir_all(p).map_err(|e| StoreError::io("create-dir", p, e))?;
        }
        Ok(())
    }

    /// Probes each root by creating and removing a temporary file; returns the roots that are not
    /// writable with the reason. An empty list means all four are writable.
    pub fn check_writable(&self) -> Vec<(RootKind, String)> {
        let mut bad = Vec::new();
        for kind in RootKind::ALL {
            let p = self.path(kind);
            let probe = fs_err::create_dir_all(p)
                .and_then(|()| tempfile::NamedTempFile::new_in(p.as_std_path()).map(drop));
            if let Err(e) = probe {
                bad.push((kind, e.to_string()));
            }
        }
        bad
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use rstest::rstest;

    use super::*;

    struct FakeEnv {
        vars: BTreeMap<&'static str, String>,
        exe: Option<Utf8PathBuf>,
    }

    impl EnvProbe for FakeEnv {
        fn var(&self, key: &str) -> Option<String> {
            self.vars.get(key).cloned()
        }
        fn home_dir(&self) -> Option<Utf8PathBuf> {
            None
        }
        fn exe_dir(&self) -> Option<Utf8PathBuf> {
            self.exe.clone()
        }
    }

    fn dirs() -> PlatformDirs {
        PlatformDirs {
            config: "/rs/config".into(),
            data: "/rs/data".into(),
            cache: "/rs/cache".into(),
            logs: "/rs/logs".into(),
            state: "/rs/state".into(),
        }
    }

    fn env(exe: Option<&str>, appimage: Option<&str>) -> FakeEnv {
        let mut vars = BTreeMap::new();
        if let Some(a) = appimage {
            vars.insert("APPIMAGE", a.to_owned());
        }
        FakeEnv {
            vars,
            exe: exe.map(Utf8PathBuf::from),
        }
    }

    #[test]
    fn without_marker_standard_dirs_are_used() {
        let r = DataRoots::resolve_with(&dirs(), &env(Some("/opt/app"), None), |_| false);
        assert!(!r.portable);
        assert_eq!(r.config, "/rs/config");
        assert_eq!(r.logs, "/rs/logs");
    }

    #[test]
    fn marker_beside_executable_moves_all_roots() {
        let r = DataRoots::resolve_with(&dirs(), &env(Some("/opt/app"), None), |p| {
            p == "/opt/app/rimstudio.portable"
        });
        assert!(r.portable);
        assert_eq!(r.config, "/opt/app/data/config");
        assert_eq!(r.data, "/opt/app/data/data");
        assert_eq!(r.cache, "/opt/app/data/cache");
        assert_eq!(r.logs, "/opt/app/data/logs");
    }

    #[test]
    fn appimage_marker_is_searched_beside_the_image() {
        let r = DataRoots::resolve_with(
            &dirs(),
            &env(
                Some("/tmp/.mount_x/usr/bin"),
                Some("/home/u/Apps/rs.AppImage"),
            ),
            |p| p == "/home/u/Apps/rimstudio.portable",
        );
        assert!(r.portable);
        assert_eq!(r.config, "/home/u/Apps/data/config");
    }

    #[test]
    fn unknown_executable_folder_means_not_portable() {
        let r = DataRoots::resolve_with(&dirs(), &env(None, None), |_| true);
        assert!(!r.portable);
    }

    #[rstest]
    #[case("/rs/config/settings.jsonc", Some(RootKind::Config))]
    #[case("/rs/data/x/y.json", Some(RootKind::Data))]
    #[case("/rs/cachefoo/x", None)]
    #[case("/elsewhere", None)]
    fn kind_of_compares_components(#[case] path: &str, #[case] expect: Option<RootKind>) {
        let r = DataRoots::from_dirs(&dirs());
        assert_eq!(r.kind_of(Utf8Path::new(path)), expect);
    }

    #[test]
    fn ensure_and_check_writable_on_a_temp_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let base = Utf8Path::from_path(tmp.path()).unwrap();
        let r = DataRoots::under_base(base);
        r.ensure_all().unwrap();
        assert!(r.check_writable().is_empty());
        for k in RootKind::ALL {
            assert!(r.path(k).is_dir());
        }
    }
}
