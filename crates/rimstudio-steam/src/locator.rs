//! Game, Steam and user folder detection.
//!
//! [`detect`] returns every candidate it can validate, each with `how` it was found and a
//! [`Confidence`], plus warnings for every defect it noticed. It never returns only the first hit
//! and never fails: a missing Steam, an unreadable file or a hung drive becomes a warning and the
//! other candidates still arrive.
//!
//! The steps, in order:
//!
//! 1. Steam roots: the override, then the candidates of [`crate::probe::steam_root_candidates`];
//!    a root is valid when `steamapps` or `config/libraryfolders.vdf` exists (on Windows also
//!    `steam.exe`); roots are deduplicated by canonical path.
//! 2. Libraries: the union of `config/libraryfolders.vdf` and `steamapps/libraryfolders.vdf` of
//!    every root plus the root itself, deduplicated by canonical path; an unreachable library is
//!    listed as offline, never dropped.
//! 3. Installs: for each online library the manifest `appmanifest_294100.acf` is read (exact file
//!    name only, empty files are reported and skipped) and its `installdir` is checked by content:
//!    an install is accepted only when `Data/Core/About/About.xml` exists. Without any manifest the
//!    folder `steamapps/common/RimWorld` is probed as a last resort. Non Steam copies (GOG and
//!    manual) are probed in well known folders and accepted by the same content test.
//! 4. User folders: every candidate that exists is listed; the one whose `Config/ModsConfig.xml`
//!    is newest is the default. The version in that file is never used as the installed version.
//! 5. Workshop content is scanned in every library and attached to each Steam install.
//!
//! Selection ranking: a valid override, then manifest confirmed Steam installs, then other Steam
//! installs, then non Steam copies; equal ranks keep detection order.

use std::collections::BTreeSet;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::os::Os;
use rimstudio_core::ports::{DetectEnv, EntryKind, PortErrorKind};
use rimstudio_core::settings::PathsSettings;
use rimstudio_core::version::GameVersion;

use crate::acf::{self, AppManifest, AppWorkshop, LibraryEntry, LibraryFolders, ManifestState};
use crate::install::{
    BUNDLE_SUFFIX, CONTENT_MARKER, DATA_DIR, GAME_APP_ID, MODS_DIR, VERSION_FILE, executable_names,
    parse_version_txt, windows_executable_names,
};
use crate::launch::describe_launch;
use crate::probe::{
    DirState, Prober, RootCandidate, UserDirCandidate, join, non_steam_install_candidates,
    norm_path, path_key, player_log_candidates, proton_user_dir, steam_root_candidates,
    user_dir_candidates,
};
use crate::report::{
    Confidence, DetectionReport, DirCheck, Health, How, Install, InstallKind, Library,
    ModsConfigInfo, REPORT_SCHEMA, RootChecks, Selected, SteamRoot, UserDir, UserDirKind,
    VersionInfo, Warning, WorkshopDirInfo, codes,
};
use crate::vdf;
use crate::workshop;

/// The default deadline of one file system call.
pub const DEFAULT_DEADLINE: Duration = Duration::from_secs(2);

/// What a detection run needs besides the environment.
#[derive(Debug, Clone)]
pub struct DetectOptions {
    /// The operating system to assume (tests run any OS on any host).
    pub os: Os,
    /// The user's overrides (the `paths` section of the workspace settings).
    pub paths: PathsSettings,
    /// A `-savedatafolder` launch override; it becomes the selected user folder when it exists.
    pub savedata_folder: Option<Utf8PathBuf>,
    /// The deadline applied to every file system call.
    pub deadline: Duration,
    /// Probe the well known folders of GOG and manual copies.
    pub include_non_steam: bool,
    /// Read `config/config.vdf` to find a Proton compatibility tool mapping.
    pub read_proton_config: bool,
    /// Extracts the game version note from the text of a `ModsConfig.xml`. XML is read only inside
    /// `rimstudio-xml`, so the caller supplies the reader (for example a closure over
    /// `rimstudio_xml::mods_config::read`). Without it `ModsConfigInfo::game_version` stays `None`
    /// and the version mismatch warning is never raised.
    pub mods_config_version: Option<fn(&str) -> Option<String>>,
}

impl DetectOptions {
    /// Options for an OS with no overrides.
    pub fn new(os: Os) -> Self {
        DetectOptions {
            os,
            paths: PathsSettings::default(),
            savedata_folder: None,
            deadline: DEFAULT_DEADLINE,
            include_non_steam: true,
            read_proton_config: true,
            mods_config_version: None,
        }
    }

    /// Replaces the overrides.
    #[must_use]
    pub fn with_paths(mut self, paths: PathsSettings) -> Self {
        self.paths = paths;
        self
    }
}

impl Default for DetectOptions {
    fn default() -> Self {
        DetectOptions::new(Os::current())
    }
}

/// Something that detects game installs.
pub trait GameLocator {
    /// Runs detection.
    fn detect(&self, env: &dyn DetectEnv, options: &DetectOptions) -> DetectionReport;
}

/// The locator for Steam, GOG and manual installs.
#[derive(Debug, Clone, Copy, Default)]
pub struct SteamLocator;

impl GameLocator for SteamLocator {
    fn detect(&self, env: &dyn DetectEnv, options: &DetectOptions) -> DetectionReport {
        detect(env, options)
    }
}

/// Runs detection. See the module documentation for the algorithm.
pub fn detect(env: &dyn DetectEnv, options: &DetectOptions) -> DetectionReport {
    let mut run = Run {
        os: options.os,
        opts: options,
        p: Prober::new(env, options.deadline),
        warnings: Vec::new(),
        forced_first: None,
    };
    let steam_roots = run.find_roots();
    let works = run.find_libraries(&steam_roots);
    let mut installs = run.find_steam_installs(&works);
    let workshop_infos = run.workshop_infos(&works);
    run.attach_workshop(&mut installs, &works, &workshop_infos);
    run.add_non_steam_and_override(&mut installs);
    let ignored: BTreeSet<&str> = options
        .paths
        .ignored_installs
        .iter()
        .map(String::as_str)
        .collect();
    installs.retain(|i| !ignored.contains(i.id.as_str()));
    let forced = run.forced_first.take();
    installs.sort_by_key(|i| rank(i, forced.as_deref()));
    if installs.len() > 1 {
        run.warn(
            codes::MULTIPLE_INSTALLS,
            format!("{} installs were found", installs.len()),
            installs.iter().map(|i| i.path.clone()).collect(),
        );
    }

    let user_dirs = run.find_user_dirs(&installs);
    let selected_user_dir = pick_user_dir(&user_dirs);
    let selected_install = installs.first().map(|i| i.id.clone());
    run.check_user_dir_version(&installs, &user_dirs, selected_user_dir.as_deref());
    if user_dirs.iter().any(|u| u.kind == UserDirKind::Proton)
        && user_dirs.iter().any(|u| u.kind == UserDirKind::Native)
    {
        run.warn(
            codes::PROTON_AND_NATIVE,
            "both a native and a Proton user folder exist".to_owned(),
            user_dirs
                .iter()
                .filter(|u| matches!(u.kind, UserDirKind::Proton | UserDirKind::Native))
                .map(|u| u.path.clone())
                .collect(),
        );
    }
    run.report_timeouts(&works);

    let mut warnings = std::mem::take(&mut run.warnings);
    warnings.sort_by(|a, b| (&a.code, &a.paths, &a.message).cmp(&(&b.code, &b.paths, &b.message)));
    warnings.dedup();
    DetectionReport {
        schema: REPORT_SCHEMA,
        generated_at_ms: env.clock().now_unix_ms(),
        os: options.os,
        steam_roots,
        libraries: works.into_iter().map(|w| w.lib).collect(),
        installs,
        user_dirs,
        selected: Selected {
            install: selected_install,
            user_dir: selected_user_dir,
        },
        warnings,
    }
}

fn rank(install: &Install, forced: Option<&str>) -> u8 {
    if forced == Some(install.id.as_str()) || install.kind == InstallKind::Override {
        0
    } else if install.kind == InstallKind::Steam && install.confidence == Confidence::High {
        1
    } else if install.kind == InstallKind::Steam {
        2
    } else {
        3
    }
}

/// The default user folder: an override, else the newest `ModsConfig.xml`, else the first.
fn pick_user_dir(dirs: &[UserDir]) -> Option<Utf8PathBuf> {
    dirs.iter()
        .find(|u| u.kind == UserDirKind::Override)
        .or_else(|| dirs.iter().find(|u| u.newest))
        .or_else(|| dirs.first())
        .map(|u| u.path.clone())
}

/// A library plus what was read from it.
struct LibWork {
    lib: Library,
    manifest: Option<AppManifest>,
}

/// What `build_install` needs to know about an accepted install folder.
struct InstallSpec<'a> {
    kind: InstallKind,
    path: Utf8PathBuf,
    game_root: Utf8PathBuf,
    how: How,
    confidence: Confidence,
    manifest: Option<&'a AppManifest>,
    library: Option<&'a Library>,
}

struct Run<'a> {
    os: Os,
    opts: &'a DetectOptions,
    p: Prober<'a>,
    warnings: Vec<Warning>,
    forced_first: Option<String>,
}

fn first_chars(text: &str) -> String {
    text.chars().take(40).collect()
}

impl Run<'_> {
    fn warn(&mut self, code: &str, message: String, paths: Vec<Utf8PathBuf>) {
        self.warnings.push(Warning {
            code: code.to_owned(),
            message,
            paths,
        });
    }

    fn norm(&self, text: &str) -> Utf8PathBuf {
        norm_path(self.os, text)
    }

    fn key(&self, path: &Utf8Path) -> String {
        path_key(self.os, path)
    }

    fn canonical_or(&self, path: &Utf8Path) -> Utf8PathBuf {
        self.p
            .canonical(self.os, path)
            .unwrap_or_else(|| path.to_path_buf())
    }

    // -----------------------------------------------------------------------------------------
    // Steam roots
    // -----------------------------------------------------------------------------------------

    fn root_checks(&self, path: &Utf8Path) -> (RootChecks, bool) {
        let exists = self.p.is_dir(path);
        let has_steamapps = exists && self.p.is_dir(&join(path, "steamapps"));
        let has_library_folders = exists && self.p.exists(&join(path, "config/libraryfolders.vdf"));
        let steam_exe = self.os == Os::Windows && exists && self.p.exists(&join(path, "steam.exe"));
        let valid = exists && (has_steamapps || has_library_folders || steam_exe);
        (
            RootChecks {
                exists,
                has_steamapps,
                has_library_folders,
            },
            valid,
        )
    }

    fn find_roots(&mut self) -> Vec<SteamRoot> {
        let mut candidates: Vec<RootCandidate> = Vec::new();
        if let Some(o) = self.opts.paths.steam_root.clone() {
            let path = self.norm(o.path.as_str());
            if self.root_checks(&path).1 {
                candidates.push(RootCandidate {
                    path,
                    how: How::Override,
                });
            } else {
                self.warn(
                    codes::OVERRIDE_INVALID,
                    "the Steam folder override is not a Steam folder".to_owned(),
                    vec![path],
                );
            }
        }
        candidates.extend(steam_root_candidates(self.os, self.p.env()));

        let mut seen = BTreeSet::new();
        let mut out = Vec::new();
        for c in candidates {
            let (checks, valid) = self.root_checks(&c.path);
            if !valid {
                if self.p.entry_kind(&c.path) == Some(EntryKind::Symlink) && !self.p.exists(&c.path)
                {
                    self.warn(
                        codes::ROOT_DANGLING_LINK,
                        "a Steam folder link points at a missing target".to_owned(),
                        vec![c.path.clone()],
                    );
                }
                continue;
            }
            let canonical = self.canonical_or(&c.path);
            if !seen.insert(self.key(&canonical)) {
                continue;
            }
            out.push(SteamRoot {
                path: c.path,
                canonical,
                how: c.how,
                confidence: if c.how == How::DirectoryProbe {
                    Confidence::Medium
                } else {
                    Confidence::High
                },
                checks,
            });
        }
        out
    }

    // -----------------------------------------------------------------------------------------
    // Libraries
    // -----------------------------------------------------------------------------------------

    /// The entries of the two library files of a root, the first file's entries first.
    fn library_entries(&mut self, root: &Utf8Path) -> Vec<LibraryEntry> {
        let mut out: Vec<LibraryEntry> = Vec::new();
        let mut seen = BTreeSet::new();
        for rel in ["config/libraryfolders.vdf", "steamapps/libraryfolders.vdf"] {
            let path = join(root, rel);
            let text = match self.p.read_text(&path) {
                Ok(t) => t,
                Err(e) => {
                    if e.kind != PortErrorKind::NotFound && e.kind != PortErrorKind::TimedOut {
                        self.warn(
                            codes::LIBRARYFOLDERS_UNREADABLE,
                            format!("a library file could not be read ({})", e.code()),
                            vec![path],
                        );
                    }
                    continue;
                }
            };
            if text
                .trim_matches(|c: char| c.is_whitespace() || c == '\0' || c == '\u{feff}')
                .is_empty()
            {
                self.warn(
                    codes::LIBRARYFOLDERS_UNREADABLE,
                    "a library file is empty".to_owned(),
                    vec![path],
                );
                continue;
            }
            match LibraryFolders::parse(&text) {
                Ok(folders) => {
                    for entry in folders.libraries {
                        let key = self.key(&self.norm(&entry.path));
                        if seen.insert(key) {
                            out.push(entry);
                        }
                    }
                }
                Err(e) => self.warn(
                    codes::LIBRARYFOLDERS_UNREADABLE,
                    format!("a library file could not be understood ({})", e.code()),
                    vec![path],
                ),
            }
        }
        out
    }

    fn find_libraries(&mut self, roots: &[SteamRoot]) -> Vec<LibWork> {
        let mut out: Vec<LibWork> = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for root in roots {
            let entries = self.library_entries(&root.canonical);
            // (path, canonical, entry) in the order they are reported.
            let mut items: Vec<(Utf8PathBuf, Utf8PathBuf, Option<LibraryEntry>)> = Vec::new();
            for entry in entries {
                let path = self.norm(&entry.path);
                let canonical = if self.p.dir_state(&path) == DirState::Present {
                    self.canonical_or(&path)
                } else {
                    path.clone()
                };
                items.push((path, canonical, Some(entry)));
            }
            let root_key = self.key(&root.canonical);
            if !items.iter().any(|(_, c, _)| self.key(c) == root_key) {
                items.insert(0, (root.canonical.clone(), root.canonical.clone(), None));
            }
            for (path, canonical, entry) in items {
                if !seen.insert(self.key(&canonical)) {
                    continue;
                }
                let work = self.read_library(&root.canonical, path, canonical, entry);
                out.push(work);
            }
        }
        out
    }

    fn read_library(
        &mut self,
        root: &Utf8Path,
        path: Utf8PathBuf,
        canonical: Utf8PathBuf,
        entry: Option<LibraryEntry>,
    ) -> LibWork {
        let listed = entry.as_ref().is_some_and(|e| e.lists_app(GAME_APP_ID));
        let mut lib = Library {
            path: path.clone(),
            canonical: canonical.clone(),
            how: if entry.is_some() {
                How::LibraryFoldersVdf
            } else {
                How::DirectoryProbe
            },
            root: root.to_path_buf(),
            online: false,
            timed_out: false,
            label: entry.as_ref().map(|e| e.label.clone()).unwrap_or_default(),
            has_app: false,
            stale: false,
        };
        match self.p.dir_state(&path) {
            DirState::TimedOut => {
                lib.timed_out = true;
                self.warn(
                    codes::LIBRARY_TIMEOUT,
                    "a Steam library did not answer in time".to_owned(),
                    vec![path],
                );
                return LibWork {
                    lib,
                    manifest: None,
                };
            }
            DirState::Missing => {
                self.warn(
                    codes::LIBRARY_OFFLINE,
                    "a Steam library folder does not exist (drive not connected?)".to_owned(),
                    vec![path],
                );
                return LibWork {
                    lib,
                    manifest: None,
                };
            }
            DirState::Present => lib.online = true,
        }

        let manifest_path = join(
            &canonical,
            &format!("steamapps/appmanifest_{GAME_APP_ID}.acf"),
        );
        let mut manifest = None;
        let mut file_exists = false;
        match self.p.read_text(&manifest_path) {
            Ok(text) => {
                file_exists = true;
                if text
                    .trim_matches(|c: char| c.is_whitespace() || c == '\0' || c == '\u{feff}')
                    .is_empty()
                {
                    self.warn(
                        codes::ACF_UNREADABLE,
                        "the app manifest is empty".to_owned(),
                        vec![manifest_path.clone()],
                    );
                } else {
                    match AppManifest::parse(&text) {
                        Ok(m) if m.app_id == GAME_APP_ID => manifest = Some(m),
                        Ok(_) => self.warn(
                            codes::ACF_UNREADABLE,
                            "the app manifest names another app".to_owned(),
                            vec![manifest_path.clone()],
                        ),
                        Err(e) => self.warn(
                            codes::ACF_UNREADABLE,
                            format!("the app manifest could not be understood ({})", e.code()),
                            vec![manifest_path.clone()],
                        ),
                    }
                }
            }
            Err(e) if e.kind == PortErrorKind::NotFound => {}
            Err(e) if e.kind == PortErrorKind::TimedOut => {}
            Err(e) => {
                file_exists = true;
                self.warn(
                    codes::ACF_UNREADABLE,
                    format!("the app manifest could not be read ({})", e.code()),
                    vec![manifest_path.clone()],
                );
            }
        }
        if let Some(entries) = self.p.read_dir(&join(&canonical, "steamapps")) {
            let prefix = format!("appmanifest_{GAME_APP_ID}.acf.");
            let leftovers: Vec<Utf8PathBuf> = entries
                .iter()
                .filter(|e| e.name.starts_with(&prefix))
                .map(|e| join(&canonical, &format!("steamapps/{}", e.name)))
                .collect();
            if !leftovers.is_empty() {
                self.warn(
                    codes::MANIFEST_LEFTOVER,
                    "temporary copies of the app manifest were ignored".to_owned(),
                    leftovers,
                );
            }
        }
        lib.has_app = manifest.is_some();
        lib.stale = listed != file_exists;
        if listed && !file_exists {
            self.warn(
                codes::APPS_TABLE_STALE,
                "the library's app list names the game but no manifest exists".to_owned(),
                vec![canonical],
            );
        }
        LibWork { lib, manifest }
    }

    // -----------------------------------------------------------------------------------------
    // Installs
    // -----------------------------------------------------------------------------------------

    /// The folder that holds `Data` and `Mods` inside an install folder, `None` when the content
    /// test fails. A macOS app bundle inside the folder is found by its `.app` suffix.
    fn game_root(&self, dir: &Utf8Path) -> Option<Utf8PathBuf> {
        if self.p.exists(&join(dir, CONTENT_MARKER)) {
            return Some(dir.to_path_buf());
        }
        let entries = self.p.read_dir(dir)?;
        entries
            .iter()
            .filter(|e| {
                matches!(e.kind, EntryKind::Dir | EntryKind::Symlink)
                    && e.name.to_lowercase().ends_with(BUNDLE_SUFFIX)
            })
            .map(|e| join(dir, &e.name))
            .find(|bundle| self.p.exists(&join(bundle, CONTENT_MARKER)))
    }

    fn build_install(&mut self, spec: InstallSpec<'_>) -> Install {
        let InstallSpec {
            kind,
            path,
            game_root,
            how,
            confidence,
            manifest,
            library,
        } = spec;
        let canonical = self.canonical_or(&path);
        let id = match library {
            Some(l) if kind == InstallKind::Steam => {
                format!("{}:{}", kind.as_str(), l.canonical)
            }
            _ => format!("{}:{}", kind.as_str(), canonical),
        };
        let is_symlink = self.p.entry_kind(&path) == Some(EntryKind::Symlink);
        if is_symlink {
            self.warn(
                codes::INSTALL_IS_SYMLINK,
                "the install folder is a symbolic link".to_owned(),
                vec![path.clone()],
            );
        }

        // The version file lives beside `Data`; an install folder next to a bundle is the fallback.
        let mut version_path = join(&game_root, VERSION_FILE);
        if game_root != path && !self.p.exists(&version_path) {
            version_path = join(&path, VERSION_FILE);
        }
        let version: Option<VersionInfo> = match self.p.read_text(&version_path) {
            Ok(text) => match parse_version_txt(&text) {
                Ok(v) => Some(VersionInfo::from(&v)),
                Err(_) => {
                    self.warn(
                        codes::VERSION_UNPARSEABLE,
                        format!("the version file holds {:?}", first_chars(text.trim())),
                        vec![version_path],
                    );
                    None
                }
            },
            Err(e) => {
                if e.kind == PortErrorKind::NotFound {
                    self.warn(
                        codes::VERSION_MISSING,
                        "the install has no version file".to_owned(),
                        vec![version_path],
                    );
                }
                None
            }
        };

        let executable = executable_names(self.os)
            .iter()
            .flat_map(|name| [join(&path, name), join(&game_root, name)])
            .find(|candidate| self.p.exists(candidate));

        let mods = join(&game_root, MODS_DIR);
        let mods_dir = DirCheck {
            exists: self.p.is_dir(&mods),
            readable: self.p.read_dir(&mods).is_some(),
            path: mods,
        };
        let mut data_dirs: Vec<String> = self
            .p
            .read_dir(&join(&game_root, DATA_DIR))
            .map(|entries| {
                entries
                    .into_iter()
                    .filter(|e| matches!(e.kind, EntryKind::Dir | EntryKind::Symlink))
                    .map(|e| e.name)
                    .collect()
            })
            .unwrap_or_default();
        data_dirs.sort();

        let health = match manifest.map(AppManifest::state) {
            Some(ManifestState::UpdatePending) => {
                self.warn(
                    codes::INSTALL_UPDATE_PENDING,
                    "Steam is updating the game; the files may not be final".to_owned(),
                    vec![path.clone()],
                );
                Health::UpdatePending
            }
            Some(ManifestState::NeedsVerify) => {
                self.warn(
                    codes::INSTALL_NEEDS_VERIFY,
                    "Steam reports the install as incomplete or damaged".to_owned(),
                    vec![path.clone()],
                );
                Health::NeedsVerify
            }
            _ => Health::Installed,
        };

        let proton =
            kind == InstallKind::Steam && library.is_some_and(|l| self.is_proton(l, &path));
        let launch = describe_launch(
            &path,
            kind == InstallKind::Steam,
            proton,
            executable.as_deref(),
        );
        Install {
            id,
            kind,
            path,
            game_root,
            how,
            confidence,
            version,
            build_id: manifest.and_then(|m| m.build_id.clone()),
            target_build_id: manifest.and_then(|m| m.target_build_id.clone()),
            state_flags: manifest.map(|m| m.state_flags),
            health,
            executable,
            launch,
            mods_dir,
            data_dirs,
            proton,
            library: library.map(|l| l.canonical.clone()),
            is_symlink,
            workshop: Vec::new(),
        }
    }

    /// The Windows build runs through Proton: a compat tool mapping for the app, a prefix with the
    /// game's user folder, or the Windows executable inside the install on a non Windows OS.
    fn is_proton(&mut self, library: &Library, install: &Utf8Path) -> bool {
        if self.os == Os::Windows || self.os == Os::MacOs {
            return false;
        }
        if self.opts.read_proton_config {
            let config = join(&library.root, "config/config.vdf");
            if let Ok(text) = self.p.read_text(&config)
                && let Ok(doc) = vdf::parse(&text)
                && acf::compat_tool(&doc, GAME_APP_ID).is_some()
            {
                return true;
            }
        }
        if self
            .p
            .is_dir(&proton_user_dir(&library.canonical, GAME_APP_ID))
        {
            return true;
        }
        windows_executable_names()
            .iter()
            .any(|n| self.p.exists(&join(install, n)))
    }

    fn find_steam_installs(&mut self, works: &[LibWork]) -> Vec<Install> {
        let mut out: Vec<Install> = Vec::new();
        let mut seen = BTreeSet::new();
        for w in works.iter().filter(|w| w.lib.online) {
            let Some(manifest) = &w.manifest else {
                continue;
            };
            let common = join(&w.lib.canonical, "steamapps/common");
            let mut names: Vec<&str> = Vec::new();
            let dir = manifest.install_dir.trim();
            let safe = !dir.is_empty() && !dir.contains(['/', '\\']) && dir != "." && dir != "..";
            if safe {
                names.push(dir);
            }
            if !names.contains(&"RimWorld") {
                names.push("RimWorld");
            }
            let mut any_dir = false;
            let mut found = None;
            for name in names {
                let candidate = join(&common, name);
                if self.p.is_dir(&candidate) {
                    any_dir = true;
                }
                if let Some(root) = self.game_root(&candidate) {
                    found = Some((candidate, root));
                    break;
                }
            }
            match found {
                Some((path, root)) => {
                    let install = self.build_install(InstallSpec {
                        kind: InstallKind::Steam,
                        path,
                        game_root: root,
                        how: How::Appmanifest,
                        confidence: Confidence::High,
                        manifest: Some(manifest),
                        library: Some(&w.lib),
                    });
                    if seen.insert(install.id.clone()) {
                        out.push(install);
                    }
                }
                None if any_dir => self.warn(
                    codes::INSTALL_CONTENT_INVALID,
                    "the install folder does not contain Data/Core/About/About.xml".to_owned(),
                    vec![join(&common, &manifest.install_dir)],
                ),
                None => self.warn(
                    codes::INSTALL_DIR_MISSING,
                    "the manifest names an install folder that does not exist".to_owned(),
                    vec![join(&common, &manifest.install_dir)],
                ),
            }
        }
        if out.is_empty() {
            // Last resort: a manually copied install in a library without a manifest.
            for w in works
                .iter()
                .filter(|w| w.lib.online && w.manifest.is_none())
            {
                let candidate = join(&w.lib.canonical, "steamapps/common/RimWorld");
                if let Some(root) = self.game_root(&candidate) {
                    let install = self.build_install(InstallSpec {
                        kind: InstallKind::Steam,
                        path: candidate,
                        game_root: root,
                        how: How::DirectoryProbe,
                        confidence: Confidence::Medium,
                        manifest: None,
                        library: Some(&w.lib),
                    });
                    if seen.insert(install.id.clone()) {
                        out.push(install);
                    }
                }
            }
        }
        out
    }

    fn add_non_steam_and_override(&mut self, installs: &mut Vec<Install>) {
        let mut canon: BTreeSet<String> = installs
            .iter()
            .map(|i| self.key(&self.canonical_or(&i.path)))
            .collect();

        let candidates = if self.opts.include_non_steam {
            non_steam_install_candidates(self.os, self.p.env())
        } else {
            Vec::new()
        };
        for path in candidates {
            let Some(root) = self.game_root(&path) else {
                continue;
            };
            let key = self.key(&self.canonical_or(&path));
            if !canon.insert(key) {
                continue;
            }
            let kind = if path.as_str().to_lowercase().contains("gog") {
                InstallKind::Gog
            } else {
                InstallKind::Manual
            };
            let install = self.build_install(InstallSpec {
                kind,
                path,
                game_root: root,
                how: How::DirectoryProbe,
                confidence: Confidence::Medium,
                manifest: None,
                library: None,
            });
            installs.push(install);
        }

        if let Some(o) = self.opts.paths.game_install.clone() {
            let path = self.norm(o.path.as_str());
            match self.game_root(&path) {
                Some(root) => {
                    let key = self.key(&self.canonical_or(&path));
                    if canon.contains(&key) {
                        self.forced_first = installs
                            .iter()
                            .find(|i| self.key(&self.canonical_or(&i.path)) == key)
                            .map(|i| i.id.clone());
                    } else {
                        let install = self.build_install(InstallSpec {
                            kind: InstallKind::Override,
                            path,
                            game_root: root,
                            how: How::Override,
                            confidence: Confidence::High,
                            manifest: None,
                            library: None,
                        });
                        canon.insert(key);
                        self.forced_first = Some(install.id.clone());
                        installs.push(install);
                    }
                }
                None => self.warn(
                    codes::OVERRIDE_INVALID,
                    "the game folder override does not contain Data/Core/About/About.xml"
                        .to_owned(),
                    vec![path],
                ),
            }
        }
    }

    // -----------------------------------------------------------------------------------------
    // Workshop
    // -----------------------------------------------------------------------------------------

    fn count_item_dirs(&self, dir: &Utf8Path) -> usize {
        self.p.read_dir(dir).map_or(0, |entries| {
            entries
                .iter()
                .filter(|e| matches!(e.kind, EntryKind::Dir | EntryKind::Symlink))
                .filter(|e| e.name.parse::<u64>().is_ok())
                .count()
        })
    }

    fn workshop_infos(&mut self, works: &[LibWork]) -> Vec<Option<WorkshopDirInfo>> {
        let mut out = Vec::with_capacity(works.len());
        for w in works {
            if !w.lib.online {
                out.push(None);
                continue;
            }
            let content = workshop::content_dir(&w.lib.canonical, GAME_APP_ID);
            let acf_file = workshop::acf_path(&w.lib.canonical, GAME_APP_ID);
            let has_content = self.p.is_dir(&content);
            let acf_text = self.p.read_text(&acf_file).ok();
            if !has_content && acf_text.is_none() {
                out.push(None);
                continue;
            }
            let items_in_acf = acf_text.as_deref().and_then(|text| {
                match vdf::parse(text)
                    .map_err(crate::error::SteamError::from)
                    .and_then(|d| AppWorkshop::from_doc(&d))
                {
                    Ok(ws) => Some(ws.installed.len()),
                    Err(e) => {
                        self.warn(
                            codes::ACF_UNREADABLE,
                            format!(
                                "the workshop manifest could not be understood ({})",
                                e.code()
                            ),
                            vec![acf_file.clone()],
                        );
                        None
                    }
                }
            });
            out.push(Some(WorkshopDirInfo {
                library: Some(w.lib.canonical.clone()),
                items_on_disk: self.count_item_dirs(&content),
                content_dir: content,
                acf: acf_text.is_some().then_some(acf_file),
                items_in_acf,
                how: How::DirectoryProbe,
            }));
        }
        out
    }

    fn attach_workshop(
        &mut self,
        installs: &mut [Install],
        works: &[LibWork],
        infos: &[Option<WorkshopDirInfo>],
    ) {
        let mut extras: Vec<WorkshopDirInfo> = Vec::new();
        for dir in self.opts.paths.extra_workshop_dirs.clone() {
            let path = self.norm(dir.as_str());
            if self.p.is_dir(&path) {
                extras.push(WorkshopDirInfo {
                    library: None,
                    items_on_disk: self.count_item_dirs(&path),
                    content_dir: path,
                    acf: None,
                    items_in_acf: None,
                    how: How::Override,
                });
            } else {
                self.warn(
                    codes::OVERRIDE_INVALID,
                    "an extra Workshop folder does not exist".to_owned(),
                    vec![path],
                );
            }
        }
        for install in installs.iter_mut().filter(|i| i.kind == InstallKind::Steam) {
            let own = install.library.clone();
            let mut list: Vec<WorkshopDirInfo> = Vec::new();
            for (w, info) in works.iter().zip(infos) {
                if let Some(info) = info
                    && Some(&w.lib.canonical) == own.as_ref()
                {
                    list.push(info.clone());
                }
            }
            let own_has = !list.is_empty();
            for (w, info) in works.iter().zip(infos) {
                if let Some(info) = info
                    && Some(&w.lib.canonical) != own.as_ref()
                {
                    list.push(info.clone());
                }
            }
            if !own_has && !list.is_empty() {
                let paths = list.iter().map(|w| w.content_dir.clone()).collect();
                self.warnings.push(Warning {
                    code: codes::WORKSHOP_DIFFERENT_LIBRARY.to_owned(),
                    message: "the Workshop content is in another library than the install"
                        .to_owned(),
                    paths,
                });
            }
            list.extend(extras.iter().cloned());
            install.workshop = list;
        }
    }

    // -----------------------------------------------------------------------------------------
    // User folders
    // -----------------------------------------------------------------------------------------

    fn find_user_dirs(&mut self, installs: &[Install]) -> Vec<UserDir> {
        let mut candidates: Vec<UserDirCandidate> = Vec::new();
        if let Some(path) = self.opts.savedata_folder.clone() {
            let path = self.norm(path.as_str());
            if self.p.is_dir(&path) {
                candidates.push(UserDirCandidate {
                    path,
                    kind: UserDirKind::Override,
                });
            } else {
                self.warn(
                    codes::OVERRIDE_INVALID,
                    "the save data folder does not exist".to_owned(),
                    vec![path],
                );
            }
        }
        if let Some(o) = self.opts.paths.user_dir.clone() {
            let path = self.norm(o.path.as_str());
            if self.p.is_dir(&path) {
                candidates.push(UserDirCandidate {
                    path,
                    kind: UserDirKind::Override,
                });
            } else {
                self.warn(
                    codes::OVERRIDE_INVALID,
                    "the user folder override does not exist".to_owned(),
                    vec![path],
                );
            }
        }
        candidates.extend(user_dir_candidates(self.os, self.p.env()));
        if matches!(self.os, Os::Linux | Os::Other) {
            let mut libs: Vec<&Utf8PathBuf> = Vec::new();
            for i in installs {
                if let Some(l) = &i.library
                    && !libs.contains(&l)
                {
                    libs.push(l);
                }
            }
            for l in libs {
                candidates.push(UserDirCandidate {
                    path: proton_user_dir(l, GAME_APP_ID),
                    kind: UserDirKind::Proton,
                });
            }
        }

        let home = self.p.env().env().home_dir();
        let mut seen = BTreeSet::new();
        let mut out: Vec<UserDir> = Vec::new();
        for c in candidates {
            if !self.p.is_dir(&c.path) {
                continue;
            }
            let config = join(&c.path, "Config/ModsConfig.xml");
            let config_exists = self.p.exists(&config);
            // The macOS folder is trusted only with a ModsConfig.xml inside.
            if c.kind == UserDirKind::Macos && !config_exists {
                continue;
            }
            let key = self.key(&self.canonical_or(&c.path));
            if !seen.insert(key) {
                continue;
            }
            let game_version = match (config_exists, self.opts.mods_config_version) {
                (true, Some(reader)) => self.p.read_text(&config).ok().and_then(|t| reader(&t)),
                _ => None,
            };
            let player_log = player_log_candidates(self.os, &c.path, home.as_deref())
                .into_iter()
                .find(|p| self.p.exists(p));
            let prefs = Some(join(&c.path, "Config/Prefs.xml")).filter(|p| self.p.exists(p));
            out.push(UserDir {
                kind: c.kind,
                mods_config: ModsConfigInfo {
                    exists: config_exists,
                    mtime_ms: if config_exists {
                        self.p.mtime_ms(&config)
                    } else {
                        None
                    },
                    game_version,
                },
                player_log,
                prefs,
                newest: false,
                path: c.path,
            });
        }
        let newest = out
            .iter()
            .enumerate()
            .filter_map(|(i, u)| Some((i, u.mods_config.mtime_ms?)))
            .fold(None::<(usize, i64)>, |best, (i, m)| match best {
                Some((_, bm)) if bm >= m => best,
                _ => Some((i, m)),
            })
            .map(|(i, _)| i);
        if let Some(i) = newest
            && let Some(u) = out.get_mut(i)
        {
            u.newest = true;
        }
        out
    }

    fn check_user_dir_version(
        &mut self,
        installs: &[Install],
        user_dirs: &[UserDir],
        selected: Option<&Utf8Path>,
    ) {
        let Some(install) = installs.first() else {
            return;
        };
        let Some(installed) = install.version.as_ref().map(|v| v.raw.as_str()) else {
            return;
        };
        let Some(dir) = selected.and_then(|s| user_dirs.iter().find(|u| u.path == s)) else {
            return;
        };
        let Some(written) = dir.mods_config.game_version.as_deref() else {
            return;
        };
        if let (Ok(a), Ok(b)) = (GameVersion::parse(installed), GameVersion::parse(written))
            && a != b
        {
            self.warn(
                codes::USERDIR_VERSION_MISMATCH,
                format!("ModsConfig.xml was written by {written}, the install is {installed}"),
                vec![dir.path.clone()],
            );
        }
    }

    // -----------------------------------------------------------------------------------------
    // Timeouts
    // -----------------------------------------------------------------------------------------

    fn report_timeouts(&mut self, works: &[LibWork]) {
        let timed_out_libs: Vec<&Utf8PathBuf> = works
            .iter()
            .filter(|w| w.lib.timed_out)
            .map(|w| &w.lib.path)
            .collect();
        for path in self.p.timeouts() {
            if timed_out_libs.iter().any(|l| path.starts_with(l.as_str())) {
                continue;
            }
            self.warn(
                codes::PROBE_TIMEOUT,
                "a file system probe did not answer in time".to_owned(),
                vec![path],
            );
        }
    }
}
