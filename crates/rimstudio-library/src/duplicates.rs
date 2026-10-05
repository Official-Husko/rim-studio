//! Duplicate package ids: grouping, choosing the effective copy, and what the game would do.
//!
//! Duplicates are a normal state for a modder. [`resolve`] groups the index by lower case package id
//! (a group of one is normal and is not reported), picks the effective entry of each group with a
//! fixed ladder, and says what the game itself would do with each copy.
//!
//! The ladder, strongest first: the entry is available (a row restored from an offline source
//! loses), an explicit pin, source rank, a `supportedVersions` match for the game version, the newest
//! `About.xml` modification time, and finally the path. The source rank puts custom folders first
//! (a lower [`ModSource::priority`](rimstudio_core::mods::ModSource) value wins, which matches the
//! list order default of custom folders), then the game `Mods` folder, then Workshop, then the
//! official `Data` folder; the class order is configurable. Nothing depends on enumeration order.
//!
//! The game's own rule (kept exactly, see [`game_treatment`]): sources load in the order official
//! `Data`, the `Mods` folder, Workshop. A second mod with an id that is already taken is accepted
//! only when exactly one of the two comes from the Workshop, in which case the Workshop copy gets
//! `_steam` appended to its id and both load; otherwise the later one is rejected with an error.
//! The game decides "later" by unsorted directory enumeration, so RimStudio uses path order and sets
//! [`DuplicateGroup::game_order_uncertain`] when that order matters.

use std::cmp::Reverse;
use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use rimstudio_core::ids::{ModIdx, SourceId};
use rimstudio_core::mods::{ModSource, SourceKind, SourceSet};
use rimstudio_core::version::GameVersion;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

use crate::index::{LibraryIndex, Loadability};

/// The default order of source classes in the choice ladder, best first.
pub const DEFAULT_CLASS_ORDER: [SourceKind; 4] = [
    SourceKind::Custom,
    SourceKind::GameMods,
    SourceKind::Workshop,
    SourceKind::GameData,
];

/// The text the game appends to the id of a Workshop copy of a duplicated id.
const STEAM_POSTFIX: &str = "_steam";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceRank {
    kind: SourceKind,
    priority: i32,
}

/// How duplicates are resolved.
#[derive(Debug, Clone)]
pub struct DuplicatePolicy {
    /// The game version used for the `supportedVersions` rung; `None` skips that rung.
    pub game_version: Option<GameVersion>,
    /// The order of source classes, best first. A class that is missing ranks last.
    pub class_order: Vec<SourceKind>,
    /// Explicit pins: lower case package id to the path of the chosen copy.
    pub pins: BTreeMap<String, Utf8PathBuf>,
    sources: BTreeMap<SourceId, SourceRank>,
}

impl Default for DuplicatePolicy {
    fn default() -> Self {
        DuplicatePolicy {
            game_version: None,
            class_order: DEFAULT_CLASS_ORDER.to_vec(),
            pins: BTreeMap::new(),
            sources: BTreeMap::new(),
        }
    }
}

impl DuplicatePolicy {
    /// A policy with the kind and priority of every source of the set.
    pub fn from_sources(sources: &SourceSet) -> DuplicatePolicy {
        let mut policy = DuplicatePolicy::default();
        for s in sources.iter() {
            policy.add_source(s);
        }
        policy
    }

    /// Records the kind and priority of one source.
    pub fn add_source(&mut self, source: &ModSource) {
        self.sources.insert(
            source.id.clone(),
            SourceRank {
                kind: source.kind,
                priority: source.priority,
            },
        );
    }

    /// Sets the game version.
    #[must_use]
    pub fn with_game_version(mut self, version: GameVersion) -> DuplicatePolicy {
        self.game_version = Some(version);
        self
    }

    /// Pins the copy of `package_id` whose mod folder is `path`.
    #[must_use]
    pub fn with_pin(mut self, package_id: &str, path: Utf8PathBuf) -> DuplicatePolicy {
        self.pins.insert(package_id.trim().to_lowercase(), path);
        self
    }

    /// Replaces the class order.
    #[must_use]
    pub fn with_class_order(mut self, order: Vec<SourceKind>) -> DuplicatePolicy {
        self.class_order = order;
        self
    }

    fn rank_of(&self, id: &SourceId) -> SourceRank {
        if let Some(r) = self.sources.get(id) {
            return *r;
        }
        let kind = if id.is_custom() {
            SourceKind::Custom
        } else if id.is_workshop() {
            SourceKind::Workshop
        } else if id.as_str() == "game-data" {
            SourceKind::GameData
        } else if id.as_str() == "game-mods" {
            SourceKind::GameMods
        } else {
            SourceKind::Custom
        };
        SourceRank { kind, priority: 0 }
    }

    fn class_rank(&self, kind: SourceKind) -> usize {
        self.class_order
            .iter()
            .position(|k| *k == kind)
            .unwrap_or(self.class_order.len())
    }
}

/// Why the effective copy won: the first rung of the ladder on which it differs from the runner up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ChoiceReason {
    /// The other copies are unavailable (their source is offline).
    Available,
    /// The user pinned this copy.
    Pinned,
    /// A better source class or a better priority within the class.
    SourcePriority,
    /// It supports the game version and the runner up does not.
    VersionMatch,
    /// Its `About.xml` is newer.
    Newer,
    /// Nothing else differed; the path order decided.
    PathOrder,
}

/// Whether a group stays inside one source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GroupScope {
    /// Every copy is in the same source (for example copies in one custom folder).
    SameSource,
    /// The copies are in at least two sources.
    CrossSource,
}

/// What the game does with one copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum GameTreatment {
    /// The game loads it under this id (the plain id, or the id with `_steam`).
    #[serde(rename_all = "camelCase")]
    Loaded {
        /// The id the game uses for the copy.
        effective_id: String,
    },
    /// The game logs an error and ignores this copy.
    Rejected,
    /// The game does not see the folder (a custom folder that is not linked).
    NotVisible,
}

/// One copy in a duplicate group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateMember {
    /// The mod in the index.
    pub idx: ModIdx,
    /// The source it was found in.
    pub source: SourceId,
    /// The kind of that source.
    pub kind: SourceKind,
    /// The mod folder.
    pub path: Utf8PathBuf,
    /// True when `supportedVersions` matches the game version (false without a version).
    pub version_match: bool,
    /// Modification time of `About.xml`.
    pub about_mtime_ns: Option<i128>,
    /// False for a row restored from an offline source.
    pub available: bool,
    /// True for the effective copy of the group.
    pub effective: bool,
    /// What the game would do with this copy.
    pub game: GameTreatment,
}

/// The explanation that the game treats a Workshop copy as `<id>_steam`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamNote {
    /// The Workshop copy.
    pub workshop: ModIdx,
    /// The id the game gives it.
    pub effective_id: String,
}

/// A set of mods that share a package id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    /// The lower case package id.
    pub key: String,
    /// The effective copy.
    pub effective: ModIdx,
    /// Why it won.
    pub reason: ChoiceReason,
    /// Whether the copies share one source.
    pub scope: GroupScope,
    /// True when the copies spell the id differently (only the case differs).
    pub spelling_differs: bool,
    /// Present when exactly one copy is a Workshop item: the game will call it `<id>_steam`.
    pub steam_note: Option<SteamNote>,
    /// True when the game's choice depends on its unsorted directory enumeration.
    pub game_order_uncertain: bool,
    /// All copies, effective first.
    pub members: Vec<DuplicateMember>,
}

/// The duplicate groups of an index.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateReport {
    /// The groups, sorted by key.
    pub groups: Vec<DuplicateGroup>,
}

impl DuplicateReport {
    /// The number of groups (not entries).
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// The number of entries in all groups.
    pub fn member_count(&self) -> usize {
        self.groups.iter().map(|g| g.members.len()).sum()
    }

    /// The group of a package id, compared ignoring case.
    pub fn group_for(&self, package_id: &str) -> Option<&DuplicateGroup> {
        let key = package_id.trim().to_lowercase();
        self.groups
            .binary_search_by(|g| g.key.as_str().cmp(key.as_str()))
            .ok()
            .and_then(|i| self.groups.get(i))
    }

    /// The effective copy of a package id, when it is duplicated.
    pub fn effective_for(&self, package_id: &str) -> Option<ModIdx> {
        self.group_for(package_id).map(|g| g.effective)
    }

    /// True when `idx` is in a group and is not its effective copy.
    pub fn is_shadowed(&self, idx: ModIdx) -> bool {
        self.groups
            .iter()
            .any(|g| g.effective != idx && g.members.iter().any(|m| m.idx == idx))
    }
}

struct Candidate {
    idx: ModIdx,
    source: SourceId,
    kind: SourceKind,
    class_rank: usize,
    priority: i32,
    path: Utf8PathBuf,
    version_match: bool,
    mtime: Option<i128>,
    available: bool,
    pinned: bool,
    visible: bool,
    spelling: String,
}

type LadderKey = (bool, bool, usize, i32, bool, Reverse<i128>, Utf8PathBuf);

fn ladder_key(c: &Candidate) -> LadderKey {
    (
        !c.available,
        !c.pinned,
        c.class_rank,
        c.priority,
        !c.version_match,
        Reverse(c.mtime.unwrap_or(i128::MIN)),
        c.path.clone(),
    )
}

/// The first rung on which the best copy beats the runner up.
fn reason_for(best: &Candidate, next: &Candidate) -> ChoiceReason {
    if best.available != next.available {
        ChoiceReason::Available
    } else if best.pinned != next.pinned {
        ChoiceReason::Pinned
    } else if best.class_rank != next.class_rank || best.priority != next.priority {
        ChoiceReason::SourcePriority
    } else if best.version_match != next.version_match {
        ChoiceReason::VersionMatch
    } else if best.mtime != next.mtime {
        ChoiceReason::Newer
    } else {
        ChoiceReason::PathOrder
    }
}

/// The class a copy has in the game's load order: official first, then `Mods`, then Workshop.
fn game_class(kind: SourceKind) -> u8 {
    match kind {
        SourceKind::GameData => 0,
        SourceKind::GameMods | SourceKind::Custom => 1,
        SourceKind::Workshop => 2,
    }
}

/// Replays the game's `TryAddMod` over the copies of one id and returns what happens to each, in
/// the order of `copies`. `copies` holds `(is_workshop, visible_to_game, class_for_order, path)`;
/// copies that are not visible are skipped, the others are added in the game's source order
/// (official, `Mods`, Workshop) and by path inside a class.
pub fn game_treatment(key: &str, copies: &[(bool, bool, SourceKind, &str)]) -> Vec<GameTreatment> {
    let mut result: Vec<GameTreatment> = vec![GameTreatment::NotVisible; copies.len()];
    let mut order: Vec<usize> = (0..copies.len())
        .filter(|&i| copies.get(i).is_some_and(|c| c.1))
        .collect();
    order.sort_by(|&a, &b| {
        let ca = copies.get(a).map_or((0, ""), |c| (game_class(c.2), c.3));
        let cb = copies.get(b).map_or((0, ""), |c| (game_class(c.2), c.3));
        ca.cmp(&cb)
    });
    let mut flagged: Vec<bool> = vec![false; copies.len()];
    let mut dict: FxHashMap<String, usize> = FxHashMap::default();
    for &m in &order {
        let mut retried = false;
        loop {
            let postfix = if flagged.get(m).copied().unwrap_or(false) {
                STEAM_POSTFIX
            } else {
                ""
            };
            let name = format!("{key}{postfix}");
            match dict.get(&name).copied() {
                Some(existing) => {
                    let m_ws = copies.get(m).is_some_and(|c| c.0);
                    let e_ws = copies.get(existing).is_some_and(|c| c.0);
                    if m_ws != e_ws && !retried {
                        let w = if m_ws { m } else { existing };
                        if let Some(f) = flagged.get_mut(w)
                            && !*f
                        {
                            *f = true;
                            retried = true;
                            continue;
                        }
                    }
                    if let Some(slot) = result.get_mut(m) {
                        *slot = GameTreatment::Rejected;
                    }
                    break;
                }
                None => {
                    dict.insert(name, m);
                    break;
                }
            }
        }
    }
    for &m in &order {
        if matches!(result.get(m), Some(GameTreatment::NotVisible))
            && let Some(slot) = result.get_mut(m)
        {
            let postfix = if flagged.get(m).copied().unwrap_or(false) {
                STEAM_POSTFIX
            } else {
                ""
            };
            *slot = GameTreatment::Loaded {
                effective_id: format!("{key}{postfix}"),
            };
        }
    }
    // A copy that was rejected stays rejected; everything visible and not rejected is loaded.
    result
}

/// Groups the index by package id and resolves every group. Deterministic: the same index and
/// policy give the same report, whatever order the scan found the mods in.
pub fn resolve(index: &LibraryIndex, policy: &DuplicatePolicy) -> DuplicateReport {
    let mut groups: BTreeMap<String, Vec<Candidate>> = BTreeMap::new();
    for (idx, meta, info, _) in index.iter_full() {
        let rank = policy.rank_of(&meta.source);
        let key = meta.package_id.lower().to_owned();
        let pinned = policy.pins.get(&key).is_some_and(|p| *p == meta.path);
        let version_match = policy
            .game_version
            .as_ref()
            .is_some_and(|v| meta.supports(v));
        groups.entry(key).or_default().push(Candidate {
            idx,
            source: meta.source.clone(),
            kind: rank.kind,
            class_rank: policy.class_rank(rank.kind),
            priority: rank.priority,
            path: meta.path.clone(),
            version_match,
            mtime: info.about_mtime_ns,
            available: info.available,
            pinned,
            visible: info.loadable == Loadability::Loadable,
            spelling: meta.package_id.as_str().to_owned(),
        });
    }

    let mut report = DuplicateReport::default();
    for (key, mut members) in groups {
        if members.len() < 2 {
            continue;
        }
        members.sort_by_key(ladder_key);
        let reason = match (members.first(), members.get(1)) {
            (Some(best), Some(next)) => reason_for(best, next),
            _ => ChoiceReason::PathOrder,
        };
        let copies: Vec<(bool, bool, SourceKind, &str)> = members
            .iter()
            .map(|m| {
                (
                    m.kind == SourceKind::Workshop,
                    m.visible,
                    m.kind,
                    m.path.as_str(),
                )
            })
            .collect();
        let treatments = game_treatment(&key, &copies);

        let workshop: Vec<&Candidate> = members
            .iter()
            .filter(|m| m.kind == SourceKind::Workshop)
            .collect();
        let steam_note = match workshop.as_slice() {
            [only] => Some(SteamNote {
                workshop: only.idx,
                effective_id: format!("{key}{STEAM_POSTFIX}"),
            }),
            _ => None,
        };
        let visible_local = members
            .iter()
            .filter(|m| {
                m.visible && m.kind != SourceKind::Workshop && m.kind != SourceKind::GameData
            })
            .count();
        let visible_workshop = members
            .iter()
            .filter(|m| m.visible && m.kind == SourceKind::Workshop)
            .count();
        let game_order_uncertain = visible_local > 1 || visible_workshop > 1;
        let first_source = members.first().map(|m| m.source.clone());
        let scope = if members
            .iter()
            .all(|m| Some(&m.source) == first_source.as_ref())
        {
            GroupScope::SameSource
        } else {
            GroupScope::CrossSource
        };
        let spelling_differs = members
            .iter()
            .any(|m| members.first().is_some_and(|f| f.spelling != m.spelling));
        let effective = members.first().map(|m| m.idx);
        let Some(effective) = effective else { continue };
        let dm: Vec<DuplicateMember> = members
            .iter()
            .zip(treatments)
            .map(|(m, game)| DuplicateMember {
                idx: m.idx,
                source: m.source.clone(),
                kind: m.kind,
                path: m.path.clone(),
                version_match: m.version_match,
                about_mtime_ns: m.mtime,
                available: m.available,
                effective: m.idx == effective,
                game,
            })
            .collect();
        report.groups.push(DuplicateGroup {
            key,
            effective,
            reason,
            scope,
            spelling_differs,
            steam_note,
            game_order_uncertain,
            members: dm,
        });
    }
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::{AboutStatus, LibraryMod, ModContent, ModInfo};
    use rimstudio_core::ids::PackageId;
    use rimstudio_core::mods::ModMeta;

    struct Spec {
        pkg: &'static str,
        source: &'static str,
        path: &'static str,
        versions: &'static [&'static str],
        mtime: Option<i128>,
        loadable: bool,
        available: bool,
    }

    fn spec(pkg: &'static str, source: &'static str, path: &'static str) -> Spec {
        Spec {
            pkg,
            source,
            path,
            versions: &[],
            mtime: None,
            loadable: !source.starts_with("cf_"),
            available: true,
        }
    }

    fn build(specs: &[Spec]) -> LibraryIndex {
        let mut b = LibraryIndex::builder();
        for s in specs {
            let mut meta = ModMeta::new(
                PackageId::parse(s.pkg).unwrap(),
                "RS Test",
                SourceId::new(s.source).unwrap(),
                Utf8PathBuf::from(s.path),
            );
            meta.supported_versions = s.versions.iter().map(|v| (*v).to_owned()).collect();
            b.push(LibraryMod {
                meta,
                info: ModInfo {
                    folder_name: s.path.rsplit('/').next().unwrap().to_owned(),
                    is_link: false,
                    available: s.available,
                    loadable: if s.loadable {
                        Loadability::Loadable
                    } else {
                        Loadability::NeedsLink
                    },
                    about: AboutStatus::Parsed,
                    synthetic_package_id: false,
                    published_file_id: None,
                    about_mtime_ns: s.mtime,
                    also_in: Vec::new(),
                },
                content: ModContent::default(),
            });
        }
        b.build()
    }

    fn effective_path(idx: &LibraryIndex, report: &DuplicateReport, pkg: &str) -> String {
        let g = report.group_for(pkg).unwrap();
        idx.get(g.effective).unwrap().path.to_string()
    }

    #[test]
    fn groups_of_one_are_not_reported() {
        let idx = build(&[
            spec("rs.a", "game-mods", "/m/a"),
            spec("rs.b", "game-mods", "/m/b"),
        ]);
        let r = resolve(&idx, &DuplicatePolicy::default());
        assert_eq!(r.group_count(), 0);
    }

    #[test]
    fn ids_are_grouped_ignoring_case_and_the_spelling_difference_is_flagged() {
        let idx = build(&[
            spec("Rs.Tree", "game-mods", "/m/a"),
            spec("rs.tree", "workshop-0", "/w/1"),
        ]);
        let r = resolve(&idx, &DuplicatePolicy::default());
        assert_eq!(r.group_count(), 1);
        let g = r.group_for("RS.TREE").unwrap();
        assert!(g.spelling_differs);
        assert_eq!(g.scope, GroupScope::CrossSource);
    }

    #[test]
    fn ladder_source_class_then_priority_then_version_then_mtime_then_path() {
        // Class: custom beats game mods beats workshop.
        let idx = build(&[
            spec("rs.x", "workshop-0", "/w/1"),
            spec("rs.x", "game-mods", "/m/x"),
            spec("rs.x", "cf_aaaaaaaa", "/c/x"),
        ]);
        let r = resolve(&idx, &DuplicatePolicy::default());
        assert_eq!(effective_path(&idx, &r, "rs.x"), "/c/x");
        assert_eq!(r.groups[0].reason, ChoiceReason::SourcePriority);

        // Priority inside custom: the lower number wins.
        let mut policy = DuplicatePolicy::default();
        let mut s1 = ModSource::new(
            SourceId::new("cf_aaaaaaaa").unwrap(),
            SourceKind::Custom,
            "/c".into(),
        );
        s1.priority = 5;
        let mut s2 = ModSource::new(
            SourceId::new("cf_bbbbbbbb").unwrap(),
            SourceKind::Custom,
            "/d".into(),
        );
        s2.priority = 1;
        policy.add_source(&s1);
        policy.add_source(&s2);
        let idx = build(&[
            spec("rs.x", "cf_aaaaaaaa", "/c/x"),
            spec("rs.x", "cf_bbbbbbbb", "/d/x"),
        ]);
        let r = resolve(&idx, &policy);
        assert_eq!(effective_path(&idx, &r, "rs.x"), "/d/x");
        assert_eq!(r.groups[0].reason, ChoiceReason::SourcePriority);

        // Version match inside one source.
        let mut a = spec("rs.v", "game-mods", "/m/a");
        a.versions = &["1.5"];
        let mut b = spec("rs.v", "game-mods", "/m/b");
        b.versions = &["1.6"];
        let idx = build(&[a, b]);
        let policy =
            DuplicatePolicy::default().with_game_version(GameVersion::parse("1.6.4000").unwrap());
        let r = resolve(&idx, &policy);
        assert_eq!(effective_path(&idx, &r, "rs.v"), "/m/b");
        assert_eq!(r.groups[0].reason, ChoiceReason::VersionMatch);

        // Newest About.xml.
        let mut a = spec("rs.n", "game-mods", "/m/a");
        a.mtime = Some(10);
        let mut b = spec("rs.n", "game-mods", "/m/b");
        b.mtime = Some(20);
        let idx = build(&[a, b]);
        let r = resolve(&idx, &DuplicatePolicy::default());
        assert_eq!(effective_path(&idx, &r, "rs.n"), "/m/b");
        assert_eq!(r.groups[0].reason, ChoiceReason::Newer);

        // Path as the last resort.
        let idx = build(&[
            spec("rs.p", "game-mods", "/m/b"),
            spec("rs.p", "game-mods", "/m/a"),
        ]);
        let r = resolve(&idx, &DuplicatePolicy::default());
        assert_eq!(effective_path(&idx, &r, "rs.p"), "/m/a");
        assert_eq!(r.groups[0].reason, ChoiceReason::PathOrder);
    }

    #[test]
    fn a_pin_beats_everything_but_availability() {
        let idx = build(&[
            spec("rs.x", "cf_aaaaaaaa", "/c/x"),
            spec("rs.x", "workshop-0", "/w/1"),
        ]);
        let policy = DuplicatePolicy::default().with_pin("RS.X", "/w/1".into());
        let r = resolve(&idx, &policy);
        assert_eq!(effective_path(&idx, &r, "rs.x"), "/w/1");
        assert_eq!(r.groups[0].reason, ChoiceReason::Pinned);
        let mut offline = spec("rs.x", "workshop-0", "/w/1");
        offline.available = false;
        let idx = build(&[spec("rs.x", "cf_aaaaaaaa", "/c/x"), offline]);
        let r = resolve(&idx, &policy);
        assert_eq!(effective_path(&idx, &r, "rs.x"), "/c/x");
        assert_eq!(r.groups[0].reason, ChoiceReason::Available);
    }

    #[test]
    fn steam_note_only_when_exactly_one_member_is_workshop() {
        let idx = build(&[
            spec("rs.t", "game-mods", "/m/t"),
            spec("rs.t", "workshop-0", "/w/1"),
        ]);
        let r = resolve(&idx, &DuplicatePolicy::default());
        let note = r.groups[0].steam_note.as_ref().unwrap();
        assert_eq!(note.effective_id, "rs.t_steam");
        let idx = build(&[
            spec("rs.t", "workshop-0", "/w/1"),
            spec("rs.t", "workshop-0", "/w/2"),
        ]);
        let r = resolve(&idx, &DuplicatePolicy::default());
        assert!(r.groups[0].steam_note.is_none());
        let idx = build(&[
            spec("rs.t", "game-mods", "/m/a"),
            spec("rs.t", "game-mods", "/m/b"),
        ]);
        let r = resolve(&idx, &DuplicatePolicy::default());
        assert!(r.groups[0].steam_note.is_none());
        assert!(r.groups[0].game_order_uncertain);
    }

    fn treat(copies: &[(bool, bool, SourceKind, &str)]) -> Vec<GameTreatment> {
        game_treatment("rs.id", copies)
    }

    fn loaded(id: &str) -> GameTreatment {
        GameTreatment::Loaded {
            effective_id: id.to_owned(),
        }
    }

    #[test]
    fn game_rule_workshop_and_local_copy_both_load_with_a_steam_postfix() {
        let t = treat(&[
            (true, true, SourceKind::Workshop, "/w/1"),
            (false, true, SourceKind::GameMods, "/m/a"),
        ]);
        assert_eq!(t, vec![loaded("rs.id_steam"), loaded("rs.id")]);
    }

    #[test]
    fn game_rule_two_local_copies_reject_the_later_one() {
        let t = treat(&[
            (false, true, SourceKind::GameMods, "/m/b"),
            (false, true, SourceKind::GameMods, "/m/a"),
        ]);
        assert_eq!(t, vec![GameTreatment::Rejected, loaded("rs.id")]);
    }

    #[test]
    fn game_rule_two_workshop_copies_reject_the_second() {
        let t = treat(&[
            (true, true, SourceKind::Workshop, "/w/2"),
            (true, true, SourceKind::Workshop, "/w/1"),
        ]);
        assert_eq!(t, vec![GameTreatment::Rejected, loaded("rs.id")]);
    }

    #[test]
    fn game_rule_official_then_mods_then_workshop_with_three_copies() {
        let t = treat(&[
            (true, true, SourceKind::Workshop, "/w/1"),
            (false, true, SourceKind::GameMods, "/m/a"),
            (false, true, SourceKind::GameData, "/d/a"),
        ]);
        // Data loads first, the Mods copy is a second plain copy and is rejected, the Workshop
        // copy gets the postfix and loads.
        assert_eq!(
            t,
            vec![
                loaded("rs.id_steam"),
                GameTreatment::Rejected,
                loaded("rs.id")
            ]
        );
    }

    #[test]
    fn game_rule_invisible_copies_are_ignored() {
        let t = treat(&[
            (false, false, SourceKind::Custom, "/c/a"),
            (false, true, SourceKind::GameMods, "/m/a"),
        ]);
        assert_eq!(t, vec![GameTreatment::NotVisible, loaded("rs.id")]);
    }

    #[test]
    fn resolve_is_independent_of_insertion_order() {
        let specs = || {
            vec![
                spec("rs.x", "workshop-0", "/w/1"),
                spec("rs.x", "game-mods", "/m/x"),
                spec("rs.y", "game-mods", "/m/y1"),
                spec("rs.y", "game-mods", "/m/y2"),
            ]
        };
        let a = resolve(&build(&specs()), &DuplicatePolicy::default());
        let mut rev = specs();
        rev.reverse();
        let b = resolve(&build(&rev), &DuplicatePolicy::default());
        assert_eq!(a, b);
        assert_eq!(a.group_count(), 2);
        assert_eq!(a.member_count(), 4);
    }

    #[test]
    fn report_helpers() {
        let idx = build(&[
            spec("rs.x", "game-mods", "/m/a"),
            spec("rs.x", "game-mods", "/m/b"),
        ]);
        let r = resolve(&idx, &DuplicatePolicy::default());
        let eff = r.effective_for("RS.X").unwrap();
        assert!(!r.is_shadowed(eff));
        let other = r.groups[0]
            .members
            .iter()
            .find(|m| !m.effective)
            .unwrap()
            .idx;
        assert!(r.is_shadowed(other));
        assert!(r.effective_for("rs.none").is_none());
    }
}
