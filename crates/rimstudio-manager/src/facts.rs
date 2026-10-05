//! What a scan reports beyond its totals: loadable and custom only counts per source, the duplicate
//! groups and the Combat Extended entry.
//!
//! Every number is computed from the scan index of the run (and its duplicate report), never from folder
//! names or workshop ids:
//!
//! - **loadable** is the library's own loadability flag of each mod: the game sees the folder (install
//!   `Data`, install `Mods`, a Steam Workshop folder, or a custom folder mod that is linked into `Mods`);
//! - **custom only** is the other flag, "needs link": the mod exists only in a custom folder;
//! - a **duplicate group** is the library's group of mods that share one package id, compared ignoring case,
//!   with the copy it keeps, the copies it skips and the rung of the choice ladder that decided;
//! - **Combat Extended** is found by looking up the package id [`CE_PACKAGE_ID`] in the index (the kept copy
//!   when there are several), and its version is the `modVersion` of that copy's `About.xml`.

use std::collections::BTreeMap;

use camino::Utf8Path;
use rimstudio_core::mods::{ModSource, SourceKind, SourceSet};
use rimstudio_core::paths::CE_PACKAGE_ID;
use rimstudio_library::duplicates::{ChoiceReason, DuplicateReport, GameTreatment, GroupScope};
use rimstudio_library::index::{LibraryIndex, Loadability};
use serde::{Deserialize, Serialize};

/// The most duplicate groups a scan result lists; the total is always reported.
pub const DUPLICATE_CAP: usize = 50;

/// Mod counts of one source.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCounts {
    /// The source id.
    pub source: String,
    /// Mods found in the source.
    pub mods: usize,
    /// Mods of the source that the game can load as they are.
    pub loadable: usize,
    /// Mods of the source that exist only in a custom folder and need a link.
    pub custom_only: usize,
}

/// One copy of a duplicated mod.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateEntry {
    /// The mod folder.
    pub path: String,
    /// The display name.
    pub name: String,
    /// The id of its source.
    pub source: String,
    /// The kind of its source.
    pub kind: SourceKind,
    /// What the game does with the copy.
    pub game: GameTreatment,
    /// Why the copy was skipped; `None` for the kept copy.
    pub why: Option<String>,
}

/// One duplicate group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFact {
    /// The package id as the kept copy spells it.
    pub package_id: String,
    /// True when every copy is in one source.
    pub same_source: bool,
    /// The rung of the choice ladder that decided.
    pub reason: ChoiceReason,
    /// The kept copy.
    pub kept: DuplicateEntry,
    /// The skipped copies, in the order of the group.
    pub skipped: Vec<DuplicateEntry>,
}

/// The duplicate groups of a scan, capped.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFacts {
    /// All groups of the scan.
    pub total: usize,
    /// All skipped copies of all groups.
    pub skipped_total: usize,
    /// The first [`DUPLICATE_CAP`] groups, sorted by package id.
    pub groups: Vec<DuplicateFact>,
}

/// Combat Extended in the scanned library.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeFact {
    /// True when the index has a mod with the Combat Extended package id.
    pub present: bool,
    /// The package id as the mod writes it.
    pub package_id: Option<String>,
    /// The display name.
    pub name: Option<String>,
    /// The mod folder.
    pub path: Option<String>,
    /// The id of its source.
    pub source: Option<String>,
    /// The kind of its source.
    pub source_kind: Option<SourceKind>,
    /// The `modVersion` of its `About.xml`.
    pub version: Option<String>,
    /// True when the game can load this copy as it stands.
    pub loadable: Option<bool>,
}

/// Everything [`gather`] reports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryFacts {
    /// Counts per source, in the order of the sources.
    pub per_source: Vec<SourceCounts>,
    /// The duplicate groups.
    pub duplicates: DuplicateFacts,
    /// Combat Extended.
    pub ce: CeFact,
}

impl LibraryFacts {
    /// The counts of a source, when it is known.
    #[must_use]
    pub fn source(&self, id: &str) -> Option<&SourceCounts> {
        self.per_source.iter().find(|s| s.source == id)
    }
}

/// The English reason that a copy lost against the kept one.
fn why_skipped(reason: ChoiceReason, game: &GameTreatment) -> String {
    let base = match reason {
        ChoiceReason::Available => "the source of this copy is offline",
        ChoiceReason::Pinned => "another copy is pinned",
        ChoiceReason::SourcePriority => "the kept copy comes from a source of higher priority",
        ChoiceReason::VersionMatch => {
            "the kept copy lists the game version as supported and this one does not"
        }
        ChoiceReason::Newer => "the About file of the kept copy is newer",
        ChoiceReason::PathOrder => "nothing else differed, so the path order decided",
    };
    match game {
        GameTreatment::Loaded { .. } => base.to_owned(),
        GameTreatment::Rejected => format!("{base}; the game logs an error and ignores it"),
        GameTreatment::NotVisible => format!("{base}; the game does not see this folder"),
    }
}

/// Reads `modVersion` from the `About.xml` of a mod folder. The folder names are matched ignoring case,
/// as the scanner does. `None` when the file is missing, unreadable or gives no version.
fn mod_version(root: &Utf8Path) -> Option<String> {
    fn find(dir: &Utf8Path, name: &str) -> Option<camino::Utf8PathBuf> {
        let direct = dir.join(name);
        if direct.exists() {
            return Some(direct);
        }
        std::fs::read_dir(dir.as_std_path())
            .ok()?
            .flatten()
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name))
            .and_then(|e| camino::Utf8PathBuf::from_path_buf(e.path()).ok())
    }
    let about_dir = find(root, "About")?;
    let file = find(&about_dir, "About.xml")?;
    let bytes = std::fs::read(file.as_std_path()).ok()?;
    rimstudio_xml::about::read_lenient(&bytes)
        .about
        .mod_version
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

fn source_kind(set: &SourceSet, id: &rimstudio_core::ids::SourceId) -> Option<SourceKind> {
    set.get(id).map(|s: &ModSource| s.kind)
}

/// Builds the facts of a scan from its index, the duplicate report of the same index and the source set.
#[must_use]
pub fn gather(index: &LibraryIndex, report: &DuplicateReport, set: &SourceSet) -> LibraryFacts {
    let mut per: BTreeMap<String, SourceCounts> = BTreeMap::new();
    for source in set.iter() {
        per.insert(
            source.id.to_string(),
            SourceCounts {
                source: source.id.to_string(),
                ..SourceCounts::default()
            },
        );
    }
    for (_, meta, info, _) in index.iter_full() {
        let entry = per
            .entry(meta.source.to_string())
            .or_insert_with(|| SourceCounts {
                source: meta.source.to_string(),
                ..SourceCounts::default()
            });
        entry.mods += 1;
        match info.loadable {
            Loadability::Loadable => entry.loadable += 1,
            Loadability::NeedsLink => entry.custom_only += 1,
        }
    }
    let mut ordered: Vec<SourceCounts> = Vec::new();
    for source in set.iter() {
        if let Some(c) = per.remove(source.id.as_str()) {
            ordered.push(c);
        }
    }
    ordered.extend(per.into_values());

    LibraryFacts {
        per_source: ordered,
        duplicates: duplicate_facts(index, report),
        ce: ce_fact(index, report, set),
    }
}

fn duplicate_facts(index: &LibraryIndex, report: &DuplicateReport) -> DuplicateFacts {
    let entry = |m: &rimstudio_library::duplicates::DuplicateMember,
                 reason: Option<ChoiceReason>| DuplicateEntry {
        path: m.path.to_string(),
        name: index.get(m.idx).map(|x| x.name.clone()).unwrap_or_default(),
        source: m.source.to_string(),
        kind: m.kind,
        game: m.game.clone(),
        why: reason.map(|r| why_skipped(r, &m.game)),
    };
    let mut out = DuplicateFacts {
        total: report.group_count(),
        ..DuplicateFacts::default()
    };
    for group in &report.groups {
        let skipped: Vec<DuplicateEntry> = group
            .members
            .iter()
            .filter(|m| !m.effective)
            .map(|m| entry(m, Some(group.reason)))
            .collect();
        out.skipped_total += skipped.len();
        if out.groups.len() >= DUPLICATE_CAP {
            continue;
        }
        let Some(kept) = group.members.iter().find(|m| m.effective) else {
            continue;
        };
        let package_id = index
            .get(kept.idx)
            .map_or_else(|| group.key.clone(), |m| m.package_id.to_string());
        out.groups.push(DuplicateFact {
            package_id,
            same_source: group.scope == GroupScope::SameSource,
            reason: group.reason,
            kept: entry(kept, None),
            skipped,
        });
    }
    out
}

fn ce_fact(index: &LibraryIndex, report: &DuplicateReport, set: &SourceSet) -> CeFact {
    let idx = report
        .effective_for(CE_PACKAGE_ID)
        .or_else(|| index.by_package_id(CE_PACKAGE_ID).first().copied());
    let Some(idx) = idx else {
        return CeFact::default();
    };
    let Some(meta) = index.get(idx) else {
        return CeFact::default();
    };
    let info = index.info(idx);
    let available = info.is_none_or(|i| i.available);
    CeFact {
        present: true,
        package_id: Some(meta.package_id.to_string()),
        name: Some(meta.name.clone()),
        path: Some(meta.path.to_string()),
        source: Some(meta.source.to_string()),
        source_kind: source_kind(set, &meta.source),
        version: if available {
            mod_version(&meta.path)
        } else {
            None
        },
        loadable: info.map(|i| i.loadable == Loadability::Loadable),
    }
}
