//! Mod sources: classifying a folder, scan layouts and overlap detection.
//!
//! A source is an ordered place where mod folders live (game `Data`, game `Mods`, a Workshop
//! content folder, or a custom folder). This module answers three questions without scanning:
//!
//! - what does this folder look like ([`classify_folder`]): a game `Data` or `Mods` folder, a
//!   Workshop folder, a folder of mods, a single mod, or nothing useful;
//! - how should a source be scanned ([`FolderSpec`], derived from a
//!   [`CustomFolder`]);
//! - does a new folder overlap an existing source ([`check_overlap`], [`probe_folder`]), which would
//!   list the same mods twice.
//!
//! The functions use the [`FsProbe`] port with a deadline, so a stalled network mount cannot hang
//! them and tests can run on a fake file system. The scanner itself reads the disk directly for
//! speed.

use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::ids::SourceId;
use rimstudio_core::mods::{ModSource, SourceKind};
use rimstudio_core::paths;
use rimstudio_core::ports::{EntryKind, FsProbe, PortError, PortErrorKind};
use rimstudio_core::settings::{
    CustomFolder, FolderLayout, MAX_SCAN_DEPTH, MIN_SCAN_DEPTH, PathRelation, path_relation,
};
use serde::{Deserialize, Serialize};

use crate::error::codes;

/// How long a probe waits for the file system before it reports the folder as unreachable.
pub const PROBE_DEADLINE: Duration = Duration::from_secs(3);
/// At most this many child folders are examined when classifying a folder.
const MAX_CHILDREN_PROBED: usize = 5000;

/// How one source is scanned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderSpec {
    /// How to interpret the folder.
    pub layout: FolderLayout,
    /// Levels below the root to search for mods, 1 to 4.
    pub depth: u8,
}

impl Default for FolderSpec {
    fn default() -> Self {
        FolderSpec {
            layout: FolderLayout::Auto,
            depth: MIN_SCAN_DEPTH,
        }
    }
}

impl FolderSpec {
    /// The built in sources: the children are mods, one level.
    pub fn mods_root() -> FolderSpec {
        FolderSpec {
            layout: FolderLayout::ModsRoot,
            depth: MIN_SCAN_DEPTH,
        }
    }

    /// The spec of a custom folder with the depth clamped to the allowed range.
    pub fn from_custom(folder: &CustomFolder) -> FolderSpec {
        FolderSpec {
            layout: folder.layout,
            depth: folder.scan_depth.clamp(MIN_SCAN_DEPTH, MAX_SCAN_DEPTH),
        }
    }

    /// The depth clamped to `1..=4`.
    pub fn clamped_depth(&self) -> u8 {
        self.depth.clamp(MIN_SCAN_DEPTH, MAX_SCAN_DEPTH)
    }
}

/// True for folder names the scanner never enters or lists as mods: hidden folders (leading dot)
/// and `node_modules`.
pub fn is_ignored_dir_name(name: &str) -> bool {
    name.starts_with('.') || name.eq_ignore_ascii_case("node_modules")
}

/// What a folder looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FolderKind {
    /// The path does not exist or cannot be reached.
    Missing,
    /// The path exists and is not a folder.
    NotDirectory,
    /// The install `Data` folder (it holds `Core`).
    GameData,
    /// The install `Mods` folder.
    GameMods,
    /// A Steam Workshop content folder for the game.
    Workshop,
    /// The folder is itself a mod.
    SingleMod,
    /// A folder whose children (or grandchildren) are mods.
    ModsRoot,
    /// A folder with no mods in it.
    Empty,
}

/// A soft problem found while classifying a folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FolderWarning {
    /// No mod was found at the depths examined.
    NoModsFound,
    /// Mods exist only below grouping folders; the scan depth must be raised.
    ModsFoundDeeper,
    /// The folder is a mod and also contains mods.
    AmbiguousLayout,
    /// The path is a link to another folder.
    PathIsLink,
    /// The folder could not be reached within the deadline.
    Unreachable,
}

/// The result of [`classify_folder`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderClass {
    /// What the folder looks like.
    pub kind: FolderKind,
    /// The number of mods found at the suggested depth.
    pub mod_count: usize,
    /// The scan depth that finds those mods.
    pub suggested_depth: u8,
    /// The layout to store for a custom folder.
    pub suggested_layout: FolderLayout,
    /// Soft problems.
    pub warnings: Vec<FolderWarning>,
}

impl FolderClass {
    fn of(kind: FolderKind) -> FolderClass {
        FolderClass {
            kind,
            mod_count: 0,
            suggested_depth: MIN_SCAN_DEPTH,
            suggested_layout: FolderLayout::Auto,
            warnings: Vec::new(),
        }
    }

    /// The source kind a folder of this class is registered as, when it is a built in place.
    pub fn source_kind(&self) -> Option<SourceKind> {
        match self.kind {
            FolderKind::GameData => Some(SourceKind::GameData),
            FolderKind::GameMods => Some(SourceKind::GameMods),
            FolderKind::Workshop => Some(SourceKind::Workshop),
            FolderKind::SingleMod | FolderKind::ModsRoot | FolderKind::Empty => {
                Some(SourceKind::Custom)
            }
            FolderKind::Missing | FolderKind::NotDirectory => None,
        }
    }
}

fn eq_ci(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// True when `dir` holds `About/About.xml` (both names matched ignoring case).
fn is_mod_dir(fs: &dyn FsProbe, dir: &Utf8Path) -> bool {
    let Ok(entries) = fs.read_dir(dir, PROBE_DEADLINE) else {
        return false;
    };
    let Some(about) = entries
        .iter()
        .find(|e| e.kind != EntryKind::File && eq_ci(&e.name, paths::ABOUT_DIR))
    else {
        return false;
    };
    let Ok(inner) = fs.read_dir(&dir.join(&about.name), PROBE_DEADLINE) else {
        return false;
    };
    inner
        .iter()
        .any(|e| e.kind != EntryKind::Dir && eq_ci(&e.name, paths::ABOUT_XML_NAME))
}

fn is_unreachable(err: &PortError) -> bool {
    err.kind == PortErrorKind::TimedOut
}

/// Looks at a folder and says what it is, how many mods it holds and which depth finds them.
///
/// Never fails: an unreachable or missing path is a [`FolderKind::Missing`] result (with the
/// [`FolderWarning::Unreachable`] warning when the cause was a timeout). The examination is bounded
/// (at most 5000 child folders per level).
pub fn classify_folder(fs: &dyn FsProbe, path: &Utf8Path) -> FolderClass {
    match fs.is_dir(path, PROBE_DEADLINE) {
        Err(e) => {
            let mut class = FolderClass::of(FolderKind::Missing);
            if is_unreachable(&e) {
                class.warnings.push(FolderWarning::Unreachable);
            }
            return class;
        }
        Ok(false) => {
            return match fs.exists(path, PROBE_DEADLINE) {
                Ok(true) => FolderClass::of(FolderKind::NotDirectory),
                _ => FolderClass::of(FolderKind::Missing),
            };
        }
        Ok(true) => {}
    }
    let mut warnings = Vec::new();
    if fs
        .metadata(path, PROBE_DEADLINE)
        .is_ok_and(|m| m.kind == EntryKind::Symlink)
    {
        warnings.push(FolderWarning::PathIsLink);
    }
    let entries = match fs.read_dir(path, PROBE_DEADLINE) {
        Ok(e) => e,
        Err(e) => {
            let mut class = FolderClass::of(FolderKind::Missing);
            if is_unreachable(&e) {
                class.warnings.push(FolderWarning::Unreachable);
            }
            return class;
        }
    };
    let children: Vec<&str> = entries
        .iter()
        .filter(|e| e.kind != EntryKind::File && !is_ignored_dir_name(&e.name))
        .map(|e| e.name.as_str())
        .take(MAX_CHILDREN_PROBED)
        .collect();

    // The folder is a mod itself.
    if is_mod_dir(fs, path) {
        let nested = children
            .iter()
            .any(|c| !eq_ci(c, paths::ABOUT_DIR) && is_mod_dir(fs, &path.join(c)));
        if nested {
            warnings.push(FolderWarning::AmbiguousLayout);
        }
        return FolderClass {
            kind: FolderKind::SingleMod,
            mod_count: 1,
            suggested_depth: MIN_SCAN_DEPTH,
            suggested_layout: FolderLayout::SingleMod,
            warnings,
        };
    }

    let name = path.file_name().unwrap_or("");
    let child_mods: Vec<&str> = children
        .iter()
        .copied()
        .filter(|c| is_mod_dir(fs, &path.join(c)))
        .collect();

    // The install Data folder: named Data and holding Core.
    if eq_ci(name, paths::DATA_DIR) && child_mods.iter().any(|c| eq_ci(c, paths::CORE_DIR)) {
        return FolderClass {
            kind: FolderKind::GameData,
            mod_count: child_mods.len(),
            suggested_depth: MIN_SCAN_DEPTH,
            suggested_layout: FolderLayout::ModsRoot,
            warnings,
        };
    }
    // The install Mods folder: named Mods and next to a Data folder that holds Core.
    if eq_ci(name, paths::MODS_DIR)
        && let Some(parent) = path.parent()
        && is_mod_dir(fs, &parent.join(paths::DATA_DIR).join(paths::CORE_DIR))
    {
        return FolderClass {
            kind: FolderKind::GameMods,
            mod_count: child_mods.len(),
            suggested_depth: MIN_SCAN_DEPTH,
            suggested_layout: FolderLayout::ModsRoot,
            warnings,
        };
    }
    // The Workshop content folder of the game: .../workshop/content/294100.
    let comps: Vec<&str> = path.components().map(|c| c.as_str()).collect();
    let app = paths::STEAM_APP_ID.to_string();
    let workshop_shaped = comps.last().is_some_and(|c| *c == app)
        && comps.len() >= 3
        && comps[comps.len() - 2] == "content"
        && comps[comps.len() - 3] == "workshop";
    if workshop_shaped {
        return FolderClass {
            kind: FolderKind::Workshop,
            mod_count: child_mods.len(),
            suggested_depth: MIN_SCAN_DEPTH,
            suggested_layout: FolderLayout::ModsRoot,
            warnings,
        };
    }

    if !child_mods.is_empty() {
        return FolderClass {
            kind: FolderKind::ModsRoot,
            mod_count: child_mods.len(),
            suggested_depth: MIN_SCAN_DEPTH,
            suggested_layout: FolderLayout::ModsRoot,
            warnings,
        };
    }
    // Mods below grouping folders.
    for depth in 2..=MAX_SCAN_DEPTH {
        let found = count_mods_at_depth(fs, path, depth);
        if found > 0 {
            warnings.push(FolderWarning::ModsFoundDeeper);
            return FolderClass {
                kind: FolderKind::ModsRoot,
                mod_count: found,
                suggested_depth: depth,
                suggested_layout: FolderLayout::ModsRoot,
                warnings,
            };
        }
    }
    warnings.push(FolderWarning::NoModsFound);
    FolderClass {
        kind: FolderKind::Empty,
        mod_count: 0,
        suggested_depth: MIN_SCAN_DEPTH,
        suggested_layout: FolderLayout::Auto,
        warnings,
    }
}

/// Counts the mods exactly `depth` levels below `dir`, never entering a mod or a link.
fn count_mods_at_depth(fs: &dyn FsProbe, dir: &Utf8Path, depth: u8) -> usize {
    let Ok(entries) = fs.read_dir(dir, PROBE_DEADLINE) else {
        return 0;
    };
    let mut total = 0;
    for e in entries
        .iter()
        .filter(|e| e.kind == EntryKind::Dir && !is_ignored_dir_name(&e.name))
        .take(MAX_CHILDREN_PROBED)
    {
        let child = dir.join(&e.name);
        if depth == 1 {
            if is_mod_dir(fs, &child) {
                total += 1;
            }
        } else if !is_mod_dir(fs, &child) {
            total += count_mods_at_depth(fs, &child, depth - 1);
        }
    }
    total
}

/// An existing source that a candidate folder overlaps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceOverlap {
    /// The source that overlaps.
    pub with: SourceId,
    /// How the candidate relates to that source's folder.
    pub relation: OverlapRelation,
    /// The diagnostic code (`deploy.source-inside-mods` or `deploy.source-overlap`).
    pub code: String,
    /// Always true today: an overlap is a hard error because the same mods would be listed twice.
    pub hard: bool,
}

/// How a candidate folder relates to an existing one (the serialisable twin of
/// [`PathRelation`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverlapRelation {
    /// The same folder.
    Same,
    /// The candidate is inside the existing folder.
    Inside,
    /// The candidate contains the existing folder.
    Contains,
}

fn fold(path: &Utf8Path, case_insensitive: bool) -> Utf8PathBuf {
    if case_insensitive {
        Utf8PathBuf::from(path.as_str().to_lowercase())
    } else {
        path.to_owned()
    }
}

/// Finds the existing sources that a candidate folder overlaps. Paths are compared lexically, so
/// canonicalise them first when links matter (see [`probe_folder`]). `case_insensitive` folds
/// case, for Windows and macOS volumes.
///
/// A candidate that equals, contains or lies inside the game `Mods` folder gets the code
/// `deploy.source-inside-mods`; any other overlap (Workshop, `Data`, another custom folder) gets
/// `deploy.source-overlap`.
pub fn check_overlap(
    candidate: &Utf8Path,
    existing: &[ModSource],
    case_insensitive: bool,
) -> Vec<SourceOverlap> {
    let cand = fold(candidate, case_insensitive);
    let mut out = Vec::new();
    for source in existing {
        let other = fold(&source.path, case_insensitive);
        let relation = match path_relation(&cand, &other) {
            PathRelation::Disjoint => continue,
            PathRelation::Same => OverlapRelation::Same,
            PathRelation::Inside => OverlapRelation::Inside,
            PathRelation::Contains => OverlapRelation::Contains,
        };
        let code = if source.kind == SourceKind::GameMods {
            codes::SOURCE_INSIDE_MODS
        } else {
            codes::SOURCE_OVERLAP
        };
        out.push(SourceOverlap {
            with: source.id.clone(),
            relation,
            code: code.as_str().to_owned(),
            hard: true,
        });
    }
    out
}

/// The result of [`probe_folder`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderProbe {
    /// What the folder looks like.
    pub class: FolderClass,
    /// The sources it overlaps.
    pub overlaps: Vec<SourceOverlap>,
    /// Hard errors and soft warnings as diagnostics.
    pub diagnostics: Vec<Diagnostic>,
    /// False when any hard error exists (relative path, missing, not a folder, overlap).
    pub can_save: bool,
}

/// Checks a folder before it is added as a source: classification plus overlap with the existing
/// sources, judged on canonical paths where the file system can resolve them.
pub fn probe_folder(
    fs: &dyn FsProbe,
    candidate: &Utf8Path,
    existing: &[ModSource],
    case_insensitive: bool,
) -> FolderProbe {
    let mut diagnostics = Vec::new();
    let mut can_save = true;
    if candidate.is_relative() {
        diagnostics.push(Diagnostic::new(
            codes::PATH_NOT_DIRECTORY,
            Severity::Error,
            "The folder path must be absolute.",
        ));
        return FolderProbe {
            class: FolderClass::of(FolderKind::Missing),
            overlaps: Vec::new(),
            diagnostics,
            can_save: false,
        };
    }
    let class = classify_folder(fs, candidate);
    match class.kind {
        FolderKind::Missing => {
            can_save = false;
            diagnostics.push(Diagnostic::new(
                codes::SOURCE_OFFLINE,
                Severity::Error,
                "The folder was not found or cannot be reached.",
            ));
        }
        FolderKind::NotDirectory => {
            can_save = false;
            diagnostics.push(Diagnostic::new(
                codes::PATH_NOT_DIRECTORY,
                Severity::Error,
                "The path is not a folder.",
            ));
        }
        _ => {}
    }
    let canon = |p: &Utf8Path| {
        fs.canonicalize(p, PROBE_DEADLINE)
            .unwrap_or_else(|_| p.to_owned())
    };
    let candidate_canon = canon(candidate);
    let canonical_existing: Vec<ModSource> = existing
        .iter()
        .map(|s| {
            let mut c = s.clone();
            c.path = canon(&s.path);
            c
        })
        .collect();
    let overlaps = check_overlap(&candidate_canon, &canonical_existing, case_insensitive);
    for o in &overlaps {
        can_save = false;
        let code = if o.code == codes::SOURCE_INSIDE_MODS.as_str() {
            codes::SOURCE_INSIDE_MODS
        } else {
            codes::SOURCE_OVERLAP
        };
        diagnostics.push(
            Diagnostic::new(
                code,
                Severity::Error,
                "The folder overlaps another source, so the same mods would be listed twice.",
            )
            .with_arg("source", o.with.as_str()),
        );
    }
    if class.warnings.contains(&FolderWarning::NoModsFound) {
        diagnostics.push(Diagnostic::new(
            codes::NO_ABOUT,
            Severity::Info,
            "No mods were found in this folder.",
        ));
    }
    FolderProbe {
        class,
        overlaps,
        diagnostics,
        can_save,
    }
}

/// Whether the game can see a mod of a source of `kind`, given the kinds of the other sources
/// that reach the same folder.
pub fn is_visible_to_game(kind: SourceKind, also_in: &[SourceKind]) -> bool {
    match kind {
        SourceKind::GameData | SourceKind::GameMods | SourceKind::Workshop => true,
        SourceKind::Custom => also_in
            .iter()
            .any(|k| matches!(k, SourceKind::GameMods | SourceKind::Workshop)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_testing::fake_fs::FakeFs;

    const ABOUT: &str = "<ModMetaData/>";

    fn fs_with_mods(root: &str, names: &[&str]) -> FakeFs {
        let fs = FakeFs::new();
        fs.add_dir(root);
        for n in names {
            fs.add_file(format!("{root}/{n}/About/About.xml"), ABOUT);
        }
        fs
    }

    #[test]
    fn missing_and_file_paths_are_classified() {
        let fs = FakeFs::new();
        fs.add_file("/f.txt", "x");
        assert_eq!(
            classify_folder(&fs, Utf8Path::new("/nope")).kind,
            FolderKind::Missing
        );
        assert_eq!(
            classify_folder(&fs, Utf8Path::new("/f.txt")).kind,
            FolderKind::NotDirectory
        );
    }

    #[test]
    fn a_timeout_is_reported_as_unreachable() {
        let fs = FakeFs::new();
        fs.add_dir("/slow");
        fs.hang("/slow");
        let c = classify_folder(&fs, Utf8Path::new("/slow"));
        assert_eq!(c.kind, FolderKind::Missing);
        assert!(c.warnings.contains(&FolderWarning::Unreachable));
    }

    #[test]
    fn a_folder_of_mods_is_a_mods_root_and_counts_them() {
        let fs = fs_with_mods("/lib", &["RS_A", "RS_B"]);
        fs.add_dir("/lib/not_a_mod/Defs");
        let c = classify_folder(&fs, Utf8Path::new("/lib"));
        assert_eq!(c.kind, FolderKind::ModsRoot);
        assert_eq!(c.mod_count, 2);
        assert_eq!(c.suggested_depth, 1);
    }

    #[test]
    fn the_about_folder_is_matched_ignoring_case() {
        let fs = FakeFs::new();
        fs.add_file("/lib/RS_A/about/about.xml", ABOUT);
        let c = classify_folder(&fs, Utf8Path::new("/lib"));
        assert_eq!(c.kind, FolderKind::ModsRoot);
        assert_eq!(c.mod_count, 1);
    }

    #[test]
    fn grouping_folders_raise_the_suggested_depth() {
        let fs = FakeFs::new();
        fs.add_file("/lib/Weapons/RS_A/About/About.xml", ABOUT);
        fs.add_file("/lib/Weapons/RS_B/About/About.xml", ABOUT);
        fs.add_file("/lib/Archive/RS_C/About/About.xml", ABOUT);
        let c = classify_folder(&fs, Utf8Path::new("/lib"));
        assert_eq!(c.kind, FolderKind::ModsRoot);
        assert_eq!(c.suggested_depth, 2);
        assert_eq!(c.mod_count, 3);
        assert!(c.warnings.contains(&FolderWarning::ModsFoundDeeper));
    }

    #[test]
    fn a_single_mod_and_the_template_repository_case() {
        let fs = FakeFs::new();
        fs.add_file("/one/About/About.xml", ABOUT);
        fs.add_file("/one/Defs/x.xml", "<Defs/>");
        assert_eq!(
            classify_folder(&fs, Utf8Path::new("/one")).kind,
            FolderKind::SingleMod
        );
        fs.add_file("/tmpl/README.md", "x");
        fs.add_file("/tmpl/template/Defs/x.xml", "<Defs/>");
        let c = classify_folder(&fs, Utf8Path::new("/tmpl"));
        assert_eq!(c.kind, FolderKind::Empty);
        assert!(c.warnings.contains(&FolderWarning::NoModsFound));
    }

    #[test]
    fn a_mod_that_contains_mods_is_flagged_ambiguous() {
        let fs = FakeFs::new();
        fs.add_file("/m/About/About.xml", ABOUT);
        fs.add_file("/m/Inner/About/About.xml", ABOUT);
        let c = classify_folder(&fs, Utf8Path::new("/m"));
        assert_eq!(c.kind, FolderKind::SingleMod);
        assert!(c.warnings.contains(&FolderWarning::AmbiguousLayout));
    }

    #[test]
    fn game_folders_are_recognised_by_shape() {
        let fs = FakeFs::new();
        fs.add_file("/g/RimWorld/Data/Core/About/About.xml", ABOUT);
        fs.add_file("/g/RimWorld/Data/Royalty/About/About.xml", ABOUT);
        fs.add_dir("/g/RimWorld/Mods");
        fs.add_file("/g/RimWorld/Mods/RS_A/About/About.xml", ABOUT);
        fs.add_file(
            "/s/steamapps/workshop/content/294100/123/About/About.xml",
            ABOUT,
        );
        assert_eq!(
            classify_folder(&fs, Utf8Path::new("/g/RimWorld/Data")).kind,
            FolderKind::GameData
        );
        assert_eq!(
            classify_folder(&fs, Utf8Path::new("/g/RimWorld/Mods")).kind,
            FolderKind::GameMods
        );
        let w = classify_folder(&fs, Utf8Path::new("/s/steamapps/workshop/content/294100"));
        assert_eq!(w.kind, FolderKind::Workshop);
        assert_eq!(w.source_kind(), Some(SourceKind::Workshop));
    }

    fn src(id: &str, kind: SourceKind, path: &str) -> ModSource {
        ModSource::new(SourceId::new(id).unwrap(), kind, Utf8PathBuf::from(path))
    }

    #[test]
    fn overlap_rules_have_distinct_codes() {
        let existing = vec![
            src("game-mods", SourceKind::GameMods, "/g/Mods"),
            src("workshop-0", SourceKind::Workshop, "/s/content/294100"),
            src("cf_aaaaaaaa", SourceKind::Custom, "/c/mine"),
        ];
        let at = |p: &str| check_overlap(Utf8Path::new(p), &existing, false);
        assert_eq!(at("/elsewhere"), vec![]);
        let o = at("/g/Mods/sub");
        assert_eq!(o[0].code, "deploy.source-inside-mods");
        assert_eq!(o[0].relation, OverlapRelation::Inside);
        let o = at("/g");
        assert_eq!(o[0].code, "deploy.source-inside-mods");
        assert_eq!(o[0].relation, OverlapRelation::Contains);
        assert_eq!(at("/s/content/294100/123")[0].code, "deploy.source-overlap");
        assert_eq!(at("/c/mine")[0].relation, OverlapRelation::Same);
        assert_eq!(at("/c")[0].relation, OverlapRelation::Contains);
        assert_eq!(at("/c/mine2"), vec![]);
    }

    #[test]
    fn case_folding_only_when_asked() {
        let existing = vec![src("cf_aaaaaaaa", SourceKind::Custom, "/Data/Mods")];
        assert!(check_overlap(Utf8Path::new("/data/mods"), &existing, false).is_empty());
        assert_eq!(
            check_overlap(Utf8Path::new("/data/mods"), &existing, true)[0].relation,
            OverlapRelation::Same
        );
    }

    #[test]
    fn probe_folder_blocks_overlaps_and_missing_paths() {
        let fs = fs_with_mods("/c/mine", &["RS_A"]);
        fs.add_dir("/c/other");
        fs.add_file("/c/other/RS_B/About/About.xml", ABOUT);
        let existing = vec![src("cf_aaaaaaaa", SourceKind::Custom, "/c/mine")];
        let ok = probe_folder(&fs, Utf8Path::new("/c/other"), &existing, false);
        assert!(ok.can_save);
        assert_eq!(ok.class.mod_count, 1);
        let dup = probe_folder(&fs, Utf8Path::new("/c/mine"), &existing, false);
        assert!(!dup.can_save);
        assert_eq!(dup.diagnostics[0].code.as_str(), "deploy.source-overlap");
        let missing = probe_folder(&fs, Utf8Path::new("/nope"), &existing, false);
        assert!(!missing.can_save);
        let rel = probe_folder(&fs, Utf8Path::new("rel/path"), &existing, false);
        assert!(!rel.can_save);
    }

    #[test]
    fn visibility_follows_the_game_rules() {
        assert!(is_visible_to_game(SourceKind::GameMods, &[]));
        assert!(!is_visible_to_game(SourceKind::Custom, &[]));
        assert!(!is_visible_to_game(
            SourceKind::Custom,
            &[SourceKind::Custom]
        ));
        assert!(is_visible_to_game(
            SourceKind::Custom,
            &[SourceKind::GameMods]
        ));
    }

    #[test]
    fn custom_folder_spec_clamps_depth() {
        let mut f = CustomFolder::new(
            SourceId::new("cf_aaaaaaaa").unwrap(),
            Utf8PathBuf::from("/c"),
        );
        f.scan_depth = 9;
        assert_eq!(FolderSpec::from_custom(&f).depth, MAX_SCAN_DEPTH);
        f.scan_depth = 0;
        assert_eq!(FolderSpec::from_custom(&f).depth, MIN_SCAN_DEPTH);
    }
}
