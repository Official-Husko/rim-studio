//! Custom mod folders as extra sources: classifying a folder by layout, counting the mods in it, and
//! checking it against the game's `Mods` folder, the Workshop content folders and the other custom
//! folders.
//!
//! The overlap checks are pure path logic ([`overlap_issues`], [`check_folder_set`]); the probing
//! functions read the file system only through [`DetectEnv`]. A path is a mod when
//! `About/About.xml` exists, with `About` and `About.xml` matched ignoring case. Scanning never
//! descends below a mod root (version folders such as `1.6` are not separate mods), skips hidden
//! folders, and does not follow symbolic links.
//!
//! Hard errors (the folder cannot be saved) and soft warnings share [`FolderIssue`]; the codes are
//! the stable identifiers the UI translates.

use std::collections::VecDeque;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::os::Os;
use rimstudio_core::ports::{DetectEnv, EntryKind, VolumeClass};
use rimstudio_core::settings::{
    CustomFolder, FolderLayout, PathRelation, normalize_path, path_relation,
};
use serde::{Deserialize, Serialize};

use crate::probe::{Prober, folds_case, join, norm_path};

/// Hard error: the folder equals, contains or lies inside the game's `Mods` folder.
pub const SOURCE_INSIDE_MODS: &str = "deploy.source-inside-mods";
/// Hard error: the folder equals, contains or lies inside a Workshop folder or another custom folder.
pub const SOURCE_OVERLAP: &str = "deploy.source-overlap";
/// Hard error: the path is not absolute.
pub const PATH_RELATIVE: &str = "scan.path-relative";
/// Hard error: the path does not exist.
pub const PATH_NOT_FOUND: &str = "scan.path-not-found";
/// Hard error: the path exists but is not a folder.
pub const PATH_NOT_DIRECTORY: &str = "scan.path-not-directory";
/// Warning: no mods were found.
pub const NO_MODS_FOUND: &str = "scan.no-mods-found";
/// Warning: the path is long for Windows without long path support.
pub const LONG_PATH_WINDOWS: &str = "scan.long-path-windows";
/// Warning: the path is a symbolic link.
pub const PATH_IS_SYMLINK: &str = "scan.path-is-symlink";
/// Warning: the folder is a mod and also holds mods.
pub const AMBIGUOUS_LAYOUT: &str = "scan.ambiguous-layout";
/// Warning: the path looks like a cloud sync folder.
pub const CLOUD_SYNC_FOLDER: &str = "scan.cloud-sync-folder";

/// The path length above which Windows needs long path support.
pub const WINDOWS_LONG_PATH: usize = 240;

/// The most folders one scan visits before it stops and reports `truncated`.
pub const MAX_DIRS_VISITED: usize = 50_000;

/// Whether an issue blocks saving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// The folder cannot be saved.
    Error,
    /// The folder can be saved.
    Warning,
}

/// A problem found while checking a folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderIssue {
    /// The stable code.
    pub code: String,
    /// Error or warning.
    pub severity: Severity,
    /// An English sentence for logs.
    pub message: String,
    /// The paths involved.
    pub paths: Vec<Utf8PathBuf>,
    /// A one line suggestion.
    pub fix: Option<String>,
}

impl FolderIssue {
    fn new(
        code: &str,
        severity: Severity,
        message: String,
        paths: Vec<Utf8PathBuf>,
        fix: Option<&str>,
    ) -> Self {
        FolderIssue {
            code: code.to_owned(),
            severity,
            message,
            paths,
            fix: fix.map(str::to_owned),
        }
    }
}

/// What kind of known root a candidate is compared with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum KnownKind {
    /// The game's own `Mods` folder.
    GameMods,
    /// A Workshop content folder.
    Workshop,
    /// Another custom folder.
    Custom,
}

/// A folder a candidate must not overlap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownRoot {
    /// The kind.
    pub kind: KnownKind,
    /// A display label.
    pub label: String,
    /// The folder.
    pub path: Utf8PathBuf,
}

/// The layout a folder resolved to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResolvedLayout {
    /// The children are mods.
    ModsRoot,
    /// The folder itself is a mod.
    SingleMod,
}

/// The result of classifying a folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderClass {
    /// The resolved layout.
    pub layout: ResolvedLayout,
    /// The mod root folders, sorted.
    pub mod_roots: Vec<Utf8PathBuf>,
    /// An `auto` folder is a mod and also holds mods (classified as a mod).
    pub ambiguous: bool,
    /// The scan stopped at [`MAX_DIRS_VISITED`].
    pub truncated: bool,
}

/// True when `dir` holds `About/About.xml` (names compared ignoring ASCII case).
pub fn is_mod_root(prober: &Prober<'_>, dir: &Utf8Path) -> bool {
    let Some(entries) = prober.read_dir(dir) else {
        return false;
    };
    let Some(about) = entries.iter().find(|e| {
        matches!(e.kind, EntryKind::Dir | EntryKind::Symlink)
            && e.name.eq_ignore_ascii_case("about")
    }) else {
        return false;
    };
    let about_dir = join(dir, &about.name);
    prober.read_dir(&about_dir).is_some_and(|inner| {
        inner
            .iter()
            .any(|e| e.kind != EntryKind::Dir && e.name.eq_ignore_ascii_case("about.xml"))
    })
}

fn find_roots(
    prober: &Prober<'_>,
    root: &Utf8Path,
    depth: u8,
    limit: usize,
) -> (Vec<Utf8PathBuf>, bool) {
    let mut found = Vec::new();
    let mut queue: VecDeque<(Utf8PathBuf, u8)> = VecDeque::new();
    queue.push_back((root.to_path_buf(), 0));
    let mut visited = 0usize;
    while let Some((dir, level)) = queue.pop_front() {
        if visited >= limit {
            found.sort();
            return (found, true);
        }
        visited += 1;
        let Some(entries) = prober.read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            if entry.kind != EntryKind::Dir || entry.name.starts_with('.') {
                continue;
            }
            let child = join(&dir, &entry.name);
            if is_mod_root(prober, &child) {
                found.push(child);
            } else if level + 1 < depth {
                queue.push_back((child, level + 1));
            }
        }
    }
    found.sort();
    (found, false)
}

fn classify_with(
    prober: &Prober<'_>,
    path: &Utf8Path,
    layout: FolderLayout,
    scan_depth: u8,
    limit: usize,
) -> FolderClass {
    let depth = scan_depth.clamp(1, 4);
    match layout {
        FolderLayout::SingleMod => FolderClass {
            layout: ResolvedLayout::SingleMod,
            mod_roots: if is_mod_root(prober, path) {
                vec![path.to_path_buf()]
            } else {
                Vec::new()
            },
            ambiguous: false,
            truncated: false,
        },
        FolderLayout::ModsRoot => {
            let (mod_roots, truncated) = find_roots(prober, path, depth, limit);
            FolderClass {
                layout: ResolvedLayout::ModsRoot,
                mod_roots,
                ambiguous: false,
                truncated,
            }
        }
        FolderLayout::Auto => {
            if is_mod_root(prober, path) {
                let (children, _) = find_roots(prober, path, 1, limit);
                FolderClass {
                    layout: ResolvedLayout::SingleMod,
                    mod_roots: vec![path.to_path_buf()],
                    ambiguous: !children.is_empty(),
                    truncated: false,
                }
            } else {
                let (mod_roots, truncated) = find_roots(prober, path, depth, limit);
                FolderClass {
                    layout: ResolvedLayout::ModsRoot,
                    mod_roots,
                    ambiguous: false,
                    truncated,
                }
            }
        }
    }
}

/// Classifies a folder by layout and lists its mod roots.
///
/// `FolderLayout::Auto` resolves to `SingleMod` when the folder itself is a mod (with `ambiguous`
/// set when it also holds mods) and to `ModsRoot` otherwise. `scan_depth` is clamped to 1 to 4:
/// 1 means the direct children are the mods, 2 allows one level of grouping folders.
pub fn classify_folder(
    env: &dyn DetectEnv,
    path: &Utf8Path,
    layout: FolderLayout,
    scan_depth: u8,
    deadline: Duration,
) -> FolderClass {
    let prober = Prober::new(env, deadline);
    classify_with(&prober, path, layout, scan_depth, MAX_DIRS_VISITED)
}

fn relation(os: Os, a: &Utf8Path, b: &Utf8Path) -> Option<PathRelation> {
    let fold = |p: &Utf8Path| -> Option<Utf8PathBuf> {
        let n = norm_path(os, p.as_str());
        let n = normalize_path(n.as_str()).ok()?;
        Some(if folds_case(os) {
            Utf8PathBuf::from(n.as_str().to_lowercase())
        } else {
            n
        })
    };
    Some(path_relation(&fold(a)?, &fold(b)?))
}

/// Checks one candidate against the folders it must not overlap. Pure path logic: pass canonical
/// paths when links matter. Equal, containing and contained folders are errors; the code is
/// [`SOURCE_INSIDE_MODS`] for the game's `Mods` folder and [`SOURCE_OVERLAP`] for the rest.
pub fn overlap_issues(os: Os, candidate: &Utf8Path, known: &[KnownRoot]) -> Vec<FolderIssue> {
    let mut out = Vec::new();
    for k in known {
        let Some(rel) = relation(os, candidate, &k.path) else {
            continue;
        };
        if rel == PathRelation::Disjoint {
            continue;
        }
        let how = match rel {
            PathRelation::Same => "is the same folder as",
            PathRelation::Inside => "lies inside",
            PathRelation::Contains => "contains",
            PathRelation::Disjoint => continue,
        };
        let (code, fix) = match k.kind {
            KnownKind::GameMods => (
                SOURCE_INSIDE_MODS,
                "Choose a folder outside the game's Mods folder",
            ),
            KnownKind::Workshop => (
                SOURCE_OVERLAP,
                "Choose a folder outside the Workshop content folder",
            ),
            KnownKind::Custom => (
                SOURCE_OVERLAP,
                "Remove the other folder or use a deeper path",
            ),
        };
        out.push(FolderIssue::new(
            code,
            Severity::Error,
            format!("{candidate} {how} {} ({})", k.path, k.label),
            vec![candidate.to_path_buf(), k.path.clone()],
            Some(fix),
        ));
    }
    out
}

/// Checks a whole list of custom folders against the game's `Mods` folder, the Workshop folders and
/// each other. Every pair is reported once, at the later folder.
pub fn check_folder_set(
    os: Os,
    folders: &[CustomFolder],
    mods_dir: Option<&Utf8Path>,
    workshop_dirs: &[Utf8PathBuf],
) -> Vec<FolderIssue> {
    let mut fixed: Vec<KnownRoot> = Vec::new();
    if let Some(m) = mods_dir {
        fixed.push(KnownRoot {
            kind: KnownKind::GameMods,
            label: "game Mods folder".to_owned(),
            path: m.to_path_buf(),
        });
    }
    for w in workshop_dirs {
        fixed.push(KnownRoot {
            kind: KnownKind::Workshop,
            label: "Workshop content".to_owned(),
            path: w.clone(),
        });
    }
    let mut out = Vec::new();
    for (i, f) in folders.iter().enumerate() {
        let mut known = fixed.clone();
        known.extend(folders.iter().take(i).map(|other| KnownRoot {
            kind: KnownKind::Custom,
            label: other.label.clone(),
            path: other.path.clone(),
        }));
        out.extend(overlap_issues(os, &f.path, &known));
    }
    out
}

/// The result of probing a folder before it is saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderProbe {
    /// The path as probed (normalised).
    pub path: Utf8PathBuf,
    /// The canonical path when it could be resolved.
    pub canonical: Option<Utf8PathBuf>,
    /// The classification, `None` when the folder does not exist.
    pub class: Option<FolderClass>,
    /// At least one mod was found.
    pub looks_like_mods_folder: bool,
    /// The number of mod roots found.
    pub mod_count: usize,
    /// The volume class of the folder.
    pub volume: VolumeClass,
    /// Errors and warnings, errors first.
    pub issues: Vec<FolderIssue>,
}

impl FolderProbe {
    /// True when no hard error was found.
    pub fn can_save(&self) -> bool {
        self.issues.iter().all(|i| i.severity != Severity::Error)
    }
}

fn looks_like_cloud_sync(path: &Utf8Path) -> bool {
    const NAMES: [&str; 5] = ["dropbox", "onedrive", "google drive", "icloud", "nextcloud"];
    let lower = path.as_str().to_lowercase();
    NAMES.iter().any(|n| lower.contains(n))
}

/// Probes a folder the user wants to add: existence, layout, mod count, volume and every hard error
/// and soft warning of the specification.
pub fn probe_folder(
    env: &dyn DetectEnv,
    os: Os,
    candidate: &Utf8Path,
    layout: FolderLayout,
    scan_depth: u8,
    known: &[KnownRoot],
    deadline: Duration,
) -> FolderProbe {
    let prober = Prober::new(env, deadline);
    let path = norm_path(os, candidate.as_str());
    let mut probe = FolderProbe {
        path: path.clone(),
        canonical: None,
        class: None,
        looks_like_mods_folder: false,
        mod_count: 0,
        volume: VolumeClass::Unknown,
        issues: Vec::new(),
    };
    let absolute = normalize_path(path.as_str()).is_ok();
    if !absolute {
        probe.issues.push(FolderIssue::new(
            PATH_RELATIVE,
            Severity::Error,
            format!("{path} is not an absolute path"),
            vec![path],
            Some("Enter the full path of the folder"),
        ));
        return probe;
    }
    if !prober.exists(&path) {
        probe.issues.push(FolderIssue::new(
            PATH_NOT_FOUND,
            Severity::Error,
            format!("{path} does not exist"),
            vec![path],
            Some("Check the path and that the drive is connected"),
        ));
        return probe;
    }
    if !prober.is_dir(&path) {
        probe.issues.push(FolderIssue::new(
            PATH_NOT_DIRECTORY,
            Severity::Error,
            format!("{path} is not a folder"),
            vec![path],
            Some("Choose the folder that holds the mods"),
        ));
        return probe;
    }

    let canonical = prober.canonical(os, &path);
    let effective = canonical.clone().unwrap_or_else(|| path.clone());
    probe.volume = env.fs().volume_class(&effective);
    probe.issues.extend(overlap_issues(os, &effective, known));

    let class = classify_with(&prober, &effective, layout, scan_depth, MAX_DIRS_VISITED);
    probe.mod_count = class.mod_roots.len();
    probe.looks_like_mods_folder = probe.mod_count > 0;
    if class.ambiguous {
        probe.issues.push(FolderIssue::new(
            AMBIGUOUS_LAYOUT,
            Severity::Warning,
            format!("{path} is a mod and also holds mods; it is treated as a single mod"),
            vec![path.clone()],
            Some("Set the layout to mods folder if the children are the mods"),
        ));
    }
    if probe.mod_count == 0 {
        probe.issues.push(FolderIssue::new(
            NO_MODS_FOUND,
            Severity::Warning,
            format!("no mods were found in {path}"),
            vec![path.clone()],
            Some("Increase the scan depth or choose another folder"),
        ));
    }
    if os == Os::Windows && path.as_str().chars().count() > WINDOWS_LONG_PATH {
        probe.issues.push(FolderIssue::new(
            LONG_PATH_WINDOWS,
            Severity::Warning,
            format!("{path} is longer than {WINDOWS_LONG_PATH} characters"),
            vec![path.clone()],
            Some("Move the folder closer to the drive root"),
        ));
    }
    if prober.entry_kind(&path) == Some(EntryKind::Symlink) {
        let target = canonical.clone().unwrap_or_else(|| path.clone());
        probe.issues.push(FolderIssue::new(
            PATH_IS_SYMLINK,
            Severity::Warning,
            format!("{path} is a link to {target}"),
            vec![path.clone(), target],
            None,
        ));
    }
    if looks_like_cloud_sync(&path) {
        probe.issues.push(FolderIssue::new(
            CLOUD_SYNC_FOLDER,
            Severity::Warning,
            format!("{path} looks like a cloud sync folder"),
            vec![path.clone()],
            Some("Sync clients can lock files while the game reads them"),
        ));
    }
    probe.canonical = canonical;
    probe.class = Some(class);
    probe.issues.sort_by_key(|i| i.severity != Severity::Error);
    probe
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::ids::SourceId;
    use rimstudio_testing::detect_env::FakeDetectEnv;
    use rstest::rstest;

    const D: Duration = Duration::from_millis(50);

    fn known(kind: KnownKind, path: &str) -> KnownRoot {
        KnownRoot {
            kind,
            label: "RS label".to_owned(),
            path: path.into(),
        }
    }

    #[rstest]
    #[case::same(Os::Linux, "/rs/mods", "/rs/mods", Some(SOURCE_INSIDE_MODS))]
    #[case::inside(Os::Linux, "/rs/mods/sub", "/rs/mods", Some(SOURCE_INSIDE_MODS))]
    #[case::contains(Os::Linux, "/rs", "/rs/mods", Some(SOURCE_INSIDE_MODS))]
    #[case::disjoint(Os::Linux, "/rs/other", "/rs/mods", None)]
    #[case::prefix_is_not_containment(Os::Linux, "/rs/mods2", "/rs/mods", None)]
    #[case::linux_is_case_sensitive(Os::Linux, "/rs/Mods", "/rs/mods", None)]
    #[case::windows_folds_case(Os::Windows, "d:\\mods", "D:/Mods", Some(SOURCE_INSIDE_MODS))]
    #[case::windows_separators(Os::Windows, "D:\\Mods\\Sub", "D:/Mods", Some(SOURCE_INSIDE_MODS))]
    #[case::mac_folds_case(Os::MacOs, "/Users/x/MODS", "/users/x/mods", Some(SOURCE_INSIDE_MODS))]
    #[case::relative_is_skipped(Os::Linux, "rs/mods", "/rs/mods", None)]
    fn overlap_with_the_game_mods_folder(
        #[case] os: Os,
        #[case] candidate: &str,
        #[case] mods: &str,
        #[case] code: Option<&str>,
    ) {
        let issues = overlap_issues(
            os,
            Utf8Path::new(candidate),
            &[known(KnownKind::GameMods, mods)],
        );
        let got: Vec<&str> = issues.iter().map(|i| i.code.as_str()).collect();
        assert_eq!(got, code.into_iter().collect::<Vec<_>>());
        assert!(
            issues
                .iter()
                .all(|i| i.severity == Severity::Error && i.fix.is_some())
        );
    }

    #[test]
    fn workshop_and_custom_overlaps_use_the_overlap_code() {
        let issues = overlap_issues(
            Os::Linux,
            Utf8Path::new("/ws/content/294100/sub"),
            &[
                known(KnownKind::Workshop, "/ws/content/294100"),
                known(KnownKind::Custom, "/ws/content"),
            ],
        );
        assert_eq!(issues.len(), 2);
        assert!(issues.iter().all(|i| i.code == SOURCE_OVERLAP));
        assert!(issues[1].fix.as_deref().unwrap().contains("deeper"));
    }

    fn folder(seed: &str, path: &str) -> CustomFolder {
        CustomFolder::new(SourceId::custom_from_seed(seed.as_bytes()), path.into())
    }

    #[test]
    fn a_folder_set_reports_each_pair_once() {
        let folders = vec![
            folder("a", "/rs/a"),
            folder("b", "/rs/a/inner"),
            folder("c", "/rs/c"),
            folder("d", "/game/Mods/x"),
        ];
        let issues = check_folder_set(
            Os::Linux,
            &folders,
            Some(Utf8Path::new("/game/Mods")),
            &[Utf8PathBuf::from("/ws/content/294100")],
        );
        let pairs: Vec<(&str, &str)> = issues
            .iter()
            .map(|i| (i.code.as_str(), i.paths[0].as_str()))
            .collect();
        assert_eq!(
            pairs,
            [
                (SOURCE_OVERLAP, "/rs/a/inner"),
                (SOURCE_INSIDE_MODS, "/game/Mods/x")
            ]
        );
    }

    fn mod_at(env: &FakeDetectEnv, dir: &str) {
        env.fs.add_file(format!("{dir}/About/About.xml"), "x");
    }

    fn tree() -> FakeDetectEnv {
        let env = FakeDetectEnv::empty(false);
        // S1: direct children are mods.
        mod_at(&env, "/rs/flat/RS_ModA");
        mod_at(&env, "/rs/flat/RS_ModB");
        env.fs.add_file("/rs/flat/RS_ModA/1.9/Defs/a.txt", "x");
        env.fs.add_file("/rs/flat/notes/readme.txt", "x");
        // S2: grouping folders.
        mod_at(&env, "/rs/grouped/RS_Group/RS_ModC");
        mod_at(&env, "/rs/grouped/RS_Group/RS_ModD");
        mod_at(&env, "/rs/grouped/RS_ModE");
        // S3: the folder is a mod.
        mod_at(&env, "/rs/single");
        env.fs.add_file("/rs/single/Defs/a.txt", "x");
        // S4: a template repository is not a mod.
        env.fs.add_file("/rs/tpl/.git/HEAD", "x");
        env.fs.add_file("/rs/tpl/Source/main.txt", "x");
        env.fs.add_file("/rs/tpl/README.md", "x");
        mod_at(&env, "/rs/tpl/RS_Real");
        // A mod that also holds a mod, with mixed case About.
        env.fs.add_file("/rs/both/about/ABOUT.XML", "x");
        mod_at(&env, "/rs/both/RS_Inner");
        // Hidden folders and links are skipped.
        mod_at(&env, "/rs/hidden/.RS_Hidden");
        env.fs.add_symlink("/rs/hidden/RS_Link", "/rs/flat/RS_ModA");
        env
    }

    fn roots(
        env: &FakeDetectEnv,
        path: &str,
        layout: FolderLayout,
        depth: u8,
    ) -> (ResolvedLayout, Vec<String>, bool) {
        let c = classify_folder(env, Utf8Path::new(path), layout, depth, D);
        (
            c.layout,
            c.mod_roots.iter().map(|p| p.to_string()).collect(),
            c.ambiguous,
        )
    }

    #[test]
    fn depth_one_finds_direct_children_and_never_descends_into_a_mod() {
        let env = tree();
        let (layout, found, _) = roots(&env, "/rs/flat", FolderLayout::Auto, 1);
        assert_eq!(layout, ResolvedLayout::ModsRoot);
        assert_eq!(found, ["/rs/flat/RS_ModA", "/rs/flat/RS_ModB"]);
    }

    #[test]
    fn depth_two_allows_grouping_folders() {
        let env = tree();
        assert_eq!(
            roots(&env, "/rs/grouped", FolderLayout::ModsRoot, 1).1,
            ["/rs/grouped/RS_ModE"]
        );
        assert_eq!(
            roots(&env, "/rs/grouped", FolderLayout::ModsRoot, 2).1,
            [
                "/rs/grouped/RS_Group/RS_ModC",
                "/rs/grouped/RS_Group/RS_ModD",
                "/rs/grouped/RS_ModE"
            ]
        );
        // The depth is clamped to 1 and 4.
        assert_eq!(
            roots(&env, "/rs/grouped", FolderLayout::ModsRoot, 0)
                .1
                .len(),
            1
        );
        assert_eq!(
            roots(&env, "/rs/grouped", FolderLayout::ModsRoot, 200)
                .1
                .len(),
            3
        );
    }

    #[test]
    fn a_single_mod_folder_is_classified_by_auto_and_by_choice() {
        let env = tree();
        assert_eq!(
            roots(&env, "/rs/single", FolderLayout::Auto, 1),
            (
                ResolvedLayout::SingleMod,
                vec!["/rs/single".to_owned()],
                false
            )
        );
        assert_eq!(
            roots(&env, "/rs/single", FolderLayout::SingleMod, 1).1,
            ["/rs/single"]
        );
        assert!(
            roots(&env, "/rs/flat", FolderLayout::SingleMod, 1)
                .1
                .is_empty()
        );
    }

    #[test]
    fn a_template_repository_is_not_a_mod_but_its_real_mod_is_found() {
        let env = tree();
        let (layout, found, _) = roots(&env, "/rs/tpl", FolderLayout::Auto, 1);
        assert_eq!(layout, ResolvedLayout::ModsRoot);
        assert_eq!(found, ["/rs/tpl/RS_Real"]);
    }

    #[test]
    fn a_mod_that_holds_mods_is_ambiguous_and_about_is_case_insensitive() {
        let env = tree();
        let (layout, found, ambiguous) = roots(&env, "/rs/both", FolderLayout::Auto, 1);
        assert_eq!(layout, ResolvedLayout::SingleMod);
        assert_eq!(found, ["/rs/both"]);
        assert!(ambiguous);
    }

    #[test]
    fn hidden_folders_and_links_are_skipped() {
        let env = tree();
        assert!(
            roots(&env, "/rs/hidden", FolderLayout::ModsRoot, 1)
                .1
                .is_empty()
        );
    }

    #[test]
    fn the_scan_stops_at_the_visit_limit() {
        let env = tree();
        let prober = Prober::new(&env, D);
        let (_, truncated) = find_roots(&prober, Utf8Path::new("/rs/grouped"), 4, 1);
        assert!(truncated);
        let (_, truncated) = find_roots(&prober, Utf8Path::new("/rs/grouped"), 4, 100);
        assert!(!truncated);
    }

    #[test]
    fn probing_reports_hard_errors_with_distinct_codes() {
        let env = tree();
        env.fs.add_file("/rs/afile.txt", "x");
        let probe = |p: &str, known: &[KnownRoot]| {
            probe_folder(
                &env,
                Os::Linux,
                Utf8Path::new(p),
                FolderLayout::Auto,
                1,
                known,
                D,
            )
        };
        assert_eq!(probe("rs/flat", &[]).issues[0].code, PATH_RELATIVE);
        assert_eq!(probe("/rs/none", &[]).issues[0].code, PATH_NOT_FOUND);
        assert_eq!(
            probe("/rs/afile.txt", &[]).issues[0].code,
            PATH_NOT_DIRECTORY
        );
        let p = probe("/rs/flat/RS_ModA", &[known(KnownKind::Custom, "/rs/flat")]);
        assert_eq!(p.issues[0].code, SOURCE_OVERLAP);
        assert!(!p.can_save());
    }

    #[test]
    fn probing_a_good_folder_counts_mods_and_warns_softly() {
        let env = tree();
        let p = probe_folder(
            &env,
            Os::Linux,
            Utf8Path::new("/rs/flat"),
            FolderLayout::Auto,
            1,
            &[],
            D,
        );
        assert!(p.can_save());
        assert!(p.looks_like_mods_folder);
        assert_eq!(p.mod_count, 2);
        assert!(p.issues.is_empty());
        assert_eq!(p.canonical.as_deref(), Some(Utf8Path::new("/rs/flat")));

        let empty = probe_folder(
            &env,
            Os::Linux,
            Utf8Path::new("/rs/flat/notes"),
            FolderLayout::Auto,
            1,
            &[],
            D,
        );
        assert!(empty.can_save());
        assert_eq!(empty.issues[0].code, NO_MODS_FOUND);
    }

    #[test]
    fn probing_notes_links_cloud_folders_and_long_windows_paths() {
        let env = tree();
        env.fs.add_symlink("/rs/linked", "/rs/flat");
        let p = probe_folder(
            &env,
            Os::Linux,
            Utf8Path::new("/rs/linked"),
            FolderLayout::Auto,
            1,
            &[],
            D,
        );
        assert!(p.issues.iter().any(|i| i.code == PATH_IS_SYMLINK));
        assert_eq!(p.mod_count, 2);

        env.fs.add_dir("/rs/Dropbox/mods");
        let p = probe_folder(
            &env,
            Os::Linux,
            Utf8Path::new("/rs/Dropbox/mods"),
            FolderLayout::Auto,
            1,
            &[],
            D,
        );
        assert!(p.issues.iter().any(|i| i.code == CLOUD_SYNC_FOLDER));

        let long = format!("D:/{}", "x".repeat(250));
        let win = FakeDetectEnv::empty(true);
        win.fs.add_dir(&long);
        let p = probe_folder(
            &win,
            Os::Windows,
            Utf8Path::new(&long),
            FolderLayout::Auto,
            1,
            &[],
            D,
        );
        assert!(p.issues.iter().any(|i| i.code == LONG_PATH_WINDOWS));

        let amb = probe_folder(
            &env,
            Os::Linux,
            Utf8Path::new("/rs/both"),
            FolderLayout::Auto,
            1,
            &[],
            D,
        );
        assert!(amb.issues.iter().any(|i| i.code == AMBIGUOUS_LAYOUT));
    }

    #[test]
    fn errors_sort_before_warnings_and_a_hung_folder_is_not_found() {
        let env = tree();
        let p = probe_folder(
            &env,
            Os::Linux,
            Utf8Path::new("/rs/flat/notes"),
            FolderLayout::Auto,
            1,
            &[known(KnownKind::GameMods, "/rs/flat")],
            D,
        );
        assert_eq!(p.issues[0].severity, Severity::Error);
        assert_eq!(p.issues.last().unwrap().severity, Severity::Warning);
        env.fs.hang("/rs/slow");
        env.fs.add_dir("/rs/slow/x");
        let p = probe_folder(
            &env,
            Os::Linux,
            Utf8Path::new("/rs/slow"),
            FolderLayout::Auto,
            1,
            &[],
            D,
        );
        assert_eq!(p.issues[0].code, PATH_NOT_FOUND);
    }

    #[test]
    fn classification_works_on_windows_style_paths() {
        let env = FakeDetectEnv::empty(true);
        env.fs.add_file("D:/Mods/RS_One/About/About.xml", "x");
        env.fs.add_file("D:/Mods/RS_Two/about/about.xml", "x");
        let c = classify_folder(&env, Utf8Path::new("D:/Mods"), FolderLayout::Auto, 1, D);
        assert_eq!(c.mod_roots.len(), 2);
    }
}
