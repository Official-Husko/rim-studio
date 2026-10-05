//! A read only scan of a mod folder: every entry with its role in the layout, and the facts about the XML
//! files the layout check needs (the weapons a file defines, whether it uses Combat Extended classes).
//!
//! The scan never follows a link, never leaves the project root and never writes. The roles come from the
//! layout rules of `docs/features/mod-layout.md` (section 4); the facts come from parsing the definition and
//! patch files with the tolerant reader of `rimstudio-xml`, so a damaged file is a finding and not a failure.

use std::collections::BTreeMap;

use rimstudio_core::tree::Node;
use rimstudio_core::version::parse_major_minor;
use rimstudio_design::plan::ProjectLayout;
use rimstudio_ipc_types::project::{NodeRoleDto, ProjectCountsDto};
use rimstudio_xml::modes::ParseMode;
use rimstudio_xml::reader::parse_document;

use crate::shared::projectfs::ProjectView;

/// The most entries a scan lists; a larger folder is cut and reported as truncated.
pub const MAX_ENTRIES: usize = 100_000;
/// The deepest folder level a scan descends to.
pub const MAX_DEPTH: usize = 16;
/// The most XML files a scan parses.
pub const MAX_PARSED_FILES: usize = 4000;
/// The largest XML file a scan parses, in bytes.
pub const MAX_PARSED_BYTES: u64 = 2_000_000;
/// The folders a scan lists but does not enter.
const NOT_ENTERED: [&str; 4] = [".git", ".vs", "node_modules", "target"];

/// One folder or file of the scan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The path relative to the project root with `/` separators.
    pub rel: String,
    /// The last segment of the path.
    pub name: String,
    /// True for a folder.
    pub is_dir: bool,
    /// The size of a file (0 for a folder and for a link).
    pub bytes: u64,
    /// The role in the layout.
    pub role: NodeRoleDto,
}

impl Entry {
    /// The path of the folder that holds the entry; empty for an entry of the project root.
    #[must_use]
    pub fn parent(&self) -> &str {
        self.rel.rsplit_once('/').map_or("", |(dir, _)| dir)
    }
}

/// A weapon definition found in a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WeaponFact {
    /// The def name.
    pub def_name: String,
    /// True for a ranged weapon (it has a verb), false for a melee weapon.
    pub ranged: bool,
    /// The explicit `techLevel` of the definition, when it has one.
    pub tech_level: Option<String>,
    /// The `texPath` of its graphic, when it has one.
    pub tex_path: Option<String>,
}

/// What a definition or patch file holds.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FileFacts {
    /// The path relative to the project root.
    pub rel: String,
    /// The root element, `None` when the file could not be parsed.
    pub root_tag: Option<String>,
    /// The weapons the file defines.
    pub weapons: Vec<WeaponFact>,
    /// The number of projectile definitions.
    pub projectiles: u32,
    /// The file mentions a Combat Extended class.
    pub ce_class: bool,
    /// The file could not be parsed (the reason is kept).
    pub unparsable: Option<String>,
}

/// The result of a scan.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Scan {
    /// Every entry, sorted by path.
    pub entries: Vec<Entry>,
    /// The facts of the parsed XML files, in path order.
    pub facts: Vec<FileFacts>,
    /// Totals.
    pub counts: ProjectCountsDto,
    /// The folder was cut at [`MAX_ENTRIES`] or [`MAX_DEPTH`].
    pub truncated: bool,
    /// The Combat Extended folder of the layout exists.
    pub ce_folder_exists: bool,
}

impl Scan {
    /// True when a folder or file with this path exists (letter case ignored).
    #[must_use]
    pub fn has_ci(&self, rel: &str) -> bool {
        self.entries.iter().any(|e| e.rel.eq_ignore_ascii_case(rel))
    }

    /// The entry with this path (letter case ignored).
    #[must_use]
    pub fn entry_ci(&self, rel: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|e| e.rel.eq_ignore_ascii_case(rel))
    }
}

fn is_container(name: &str) -> bool {
    name == "Common" || parse_major_minor(name, false).is_some_and(|_| is_plain_version(name))
}

/// `1.6` yes, `1.6NotOdyssey` and `1.6.2` no: a version folder is exactly `major.minor`.
fn is_plain_version(name: &str) -> bool {
    let mut parts = name.split('.');
    let (Some(a), Some(b), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !a.is_empty()
        && !b.is_empty()
        && a.bytes().all(|c| c.is_ascii_digit())
        && b.bytes().all(|c| c.is_ascii_digit())
}

fn within(rel: &str, folder: &str) -> bool {
    let (rel, folder) = (rel.to_ascii_lowercase(), folder.to_ascii_lowercase());
    rel == folder || rel.starts_with(&format!("{folder}/"))
}

/// The role of a path below a content folder (the project root, a version folder or `Common`).
fn content_role(segs: &[&str], layout: &ProjectLayout, ce_exists: bool) -> NodeRoleDto {
    let Some(first) = segs.first() else {
        return NodeRoleDto::Other;
    };
    let rel = segs.join("/");
    if within(&rel, &layout.ce_folder)
        || (ce_exists
            && layout
                .ce_folder
                .to_ascii_lowercase()
                .starts_with(&format!("{}/", rel.to_ascii_lowercase())))
    {
        return NodeRoleDto::CeCompat;
    }
    match first.to_ascii_lowercase().as_str() {
        "defs" => {
            if let Some(sub) = segs.get(1..).filter(|s| !s.is_empty()) {
                let sub = sub.join("/");
                if !layout.weapons_dir.is_empty() && within(&sub, &layout.weapons_dir) {
                    return NodeRoleDto::DefsWeapons;
                }
                if within(&sub, rimstudio_core::paths::DEFS_SOUNDS_DIR) {
                    return NodeRoleDto::DefsSounds;
                }
            }
            NodeRoleDto::Defs
        }
        "patches" => NodeRoleDto::Patches,
        "textures" => NodeRoleDto::Textures,
        "sounds" => NodeRoleDto::Sounds,
        "languages" => NodeRoleDto::Languages,
        "assemblies" => NodeRoleDto::Assemblies,
        _ => NodeRoleDto::Other,
    }
}

/// The role of a path relative to the project root.
#[must_use]
pub fn role_of(rel: &str, layout: &ProjectLayout, ce_exists: bool) -> NodeRoleDto {
    let segs: Vec<&str> = rel.split('/').filter(|s| !s.is_empty()).collect();
    let Some(first) = segs.first() else {
        return NodeRoleDto::Other;
    };
    if first.eq_ignore_ascii_case("About") {
        return NodeRoleDto::About;
    }
    if segs.len() == 1 && first.eq_ignore_ascii_case("LoadFolders.xml") {
        return NodeRoleDto::LoadFolders;
    }
    if first.eq_ignore_ascii_case("Source") || first.eq_ignore_ascii_case("Raw Assets") {
        return NodeRoleDto::Source;
    }
    if is_container(first) {
        return match segs.get(1..).filter(|s| !s.is_empty()) {
            None => NodeRoleDto::ContentRoot,
            Some(rest) => content_role(rest, layout, ce_exists),
        };
    }
    content_role(&segs, layout, ce_exists)
}

/// True when the path is inside the content folder the layout serves (the project root has no version
/// folder; otherwise the version folder or `Common`).
#[must_use]
pub fn in_active_content(rel: &str, layout: &ProjectLayout) -> bool {
    let first = rel.split('/').next().unwrap_or("");
    match &layout.version_folder {
        Some(v) => first == v && rel.len() > v.len(),
        None => !is_container(first),
    }
}

fn is_xml(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("xml"))
}

fn is_weapon(def: &Node) -> Option<WeaponFact> {
    if def
        .attr("Abstract")
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
    {
        return None;
    }
    let def_name = def.child_text("defName")?.trim().to_owned();
    let ranged = def.child("verbs").is_some_and(|verbs| {
        verbs
            .elements()
            .any(|li| li.child("verbClass").is_some() || li.child("defaultProjectile").is_some())
    });
    let tagged = def.child("weaponTags").is_some() || def.child("weaponClasses").is_some();
    let parent = def.attr("ParentName").unwrap_or("");
    let melee_like = def.child("tools").is_some()
        && def.child("race").is_none()
        && (parent.contains("Melee") || parent.contains("Weapon"));
    if !(ranged || tagged || melee_like) {
        return None;
    }
    Some(WeaponFact {
        def_name,
        ranged,
        tech_level: def
            .child_text("techLevel")
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty()),
        tex_path: def
            .find("graphicData/texPath")
            .and_then(Node::leaf_text)
            .map(|t| t.trim().to_owned())
            .filter(|t| !t.is_empty()),
    })
}

fn facts_of(rel: &str, bytes: &[u8]) -> FileFacts {
    let mut facts = FileFacts {
        rel: rel.to_owned(),
        ce_class: String::from_utf8_lossy(bytes)
            .contains(rimstudio_design::ce::patchgen::export::CE_MARK),
        ..FileFacts::default()
    };
    match parse_document(bytes, ParseMode::Game) {
        Err(e) => facts.unparsable = Some(e.to_string()),
        Ok(doc) => {
            facts.root_tag = Some(doc.root.tag.clone());
            if doc.root.tag == "Defs" {
                for def in doc.root.children_named("ThingDef") {
                    if def.child("projectile").is_some() {
                        facts.projectiles = facts.projectiles.saturating_add(1);
                    }
                    if let Some(w) = is_weapon(def) {
                        facts.weapons.push(w);
                    }
                }
            }
        }
    }
    facts
}

struct Walk<'a> {
    layout: &'a ProjectLayout,
    ce_exists: bool,
    entries: Vec<Entry>,
    xml: Vec<(String, std::path::PathBuf, u64)>,
    truncated: bool,
}

impl Walk<'_> {
    fn dir(&mut self, abs: &std::path::Path, rel: &str, depth: usize) {
        let Ok(read) = std::fs::read_dir(abs) else {
            return;
        };
        let mut children: Vec<_> = read.flatten().collect();
        children.sort_by_key(std::fs::DirEntry::file_name);
        for entry in children {
            if self.entries.len() >= MAX_ENTRIES {
                self.truncated = true;
                return;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let child_rel = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            let role = role_of(&child_rel, self.layout, self.ce_exists);
            if kind.is_dir() {
                self.entries.push(Entry {
                    rel: child_rel.clone(),
                    name: name.clone(),
                    is_dir: true,
                    bytes: 0,
                    role,
                });
                if NOT_ENTERED.contains(&name.as_str()) {
                    continue;
                }
                if depth >= MAX_DEPTH {
                    self.truncated = true;
                    continue;
                }
                self.dir(&entry.path(), &child_rel, depth + 1);
            } else {
                // a link is listed as an empty file and never followed
                let bytes = if kind.is_file() {
                    entry.metadata().map_or(0, |m| m.len())
                } else {
                    0
                };
                if kind.is_file() && is_xml(&name) && holds_defs_or_patches(role) {
                    self.xml.push((child_rel.clone(), entry.path(), bytes));
                }
                self.entries.push(Entry {
                    rel: child_rel,
                    name,
                    is_dir: false,
                    bytes,
                    role,
                });
            }
        }
    }
}

/// Roles whose XML files are definition or patch files.
fn holds_defs_or_patches(role: NodeRoleDto) -> bool {
    matches!(
        role,
        NodeRoleDto::Defs
            | NodeRoleDto::DefsWeapons
            | NodeRoleDto::DefsSounds
            | NodeRoleDto::Patches
            | NodeRoleDto::CeCompat
    )
}

/// Scans a project folder.
#[must_use]
pub fn scan(view: &ProjectView) -> Scan {
    let layout = &view.layout;
    let root = view.root.as_std_path();
    let ce_dir = layout.ce_dir();
    let ce_exists = root.join(&ce_dir).is_dir();
    let mut walk = Walk {
        layout,
        ce_exists,
        entries: Vec::new(),
        xml: Vec::new(),
        truncated: false,
    };
    walk.dir(root, "", 0);
    let Walk {
        mut entries,
        xml,
        truncated,
        ..
    } = walk;
    entries.sort_by(|a, b| a.rel.cmp(&b.rel));

    let mut facts = Vec::new();
    for (rel, path, bytes) in xml.into_iter().take(MAX_PARSED_FILES) {
        if bytes > MAX_PARSED_BYTES {
            continue;
        }
        let Ok(data) = std::fs::read(&path) else {
            continue;
        };
        facts.push(facts_of(&rel, &data));
    }
    facts.sort_by(|a, b| a.rel.cmp(&b.rel));

    let mut counts = ProjectCountsDto::default();
    let mut by_rel: BTreeMap<&str, &FileFacts> = BTreeMap::new();
    for f in &facts {
        by_rel.insert(f.rel.as_str(), f);
    }
    for e in &entries {
        if e.is_dir {
            counts.folders = counts.folders.saturating_add(1);
            continue;
        }
        counts.files = counts.files.saturating_add(1);
        counts.bytes = counts.bytes.saturating_add(e.bytes);
        let xml = is_xml(&e.name);
        match e.role {
            NodeRoleDto::Defs | NodeRoleDto::DefsWeapons | NodeRoleDto::DefsSounds if xml => {
                counts.def_files = counts.def_files.saturating_add(1);
            }
            NodeRoleDto::Patches | NodeRoleDto::CeCompat if xml => {
                counts.patch_files = counts.patch_files.saturating_add(1);
            }
            NodeRoleDto::Textures => counts.textures = counts.textures.saturating_add(1),
            NodeRoleDto::Sounds => counts.sounds = counts.sounds.saturating_add(1),
            _ => {}
        }
        if let Some(f) = by_rel.get(e.rel.as_str())
            && f.root_tag.as_deref() == Some("Defs")
        {
            counts.weapon_defs = counts
                .weapon_defs
                .saturating_add(u32::try_from(f.weapons.len()).unwrap_or(u32::MAX));
            counts.projectile_defs = counts.projectile_defs.saturating_add(f.projectiles);
        }
    }
    Scan {
        entries,
        facts,
        counts,
        truncated,
        ce_folder_exists: ce_exists,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_design::plan::LayoutProfile;

    #[test]
    fn roles_follow_the_layout() {
        let l = ProjectLayout::default();
        let r = |p: &str| role_of(p, &l, true);
        assert_eq!(r("About/About.xml"), NodeRoleDto::About);
        assert_eq!(r("LoadFolders.xml"), NodeRoleDto::LoadFolders);
        assert_eq!(r("Defs"), NodeRoleDto::Defs);
        assert_eq!(r("Defs/ThingDefs_Misc"), NodeRoleDto::Defs);
        assert_eq!(
            r("Defs/ThingDefs_Misc/Weapons/RangedIndustrial/A.xml"),
            NodeRoleDto::DefsWeapons
        );
        assert_eq!(r("Defs/SoundDefs/A.xml"), NodeRoleDto::DefsSounds);
        assert_eq!(r("Patches/A.xml"), NodeRoleDto::Patches);
        assert_eq!(r("Compat"), NodeRoleDto::CeCompat);
        assert_eq!(
            r("Compat/CombatExtended/Patches/A.xml"),
            NodeRoleDto::CeCompat
        );
        assert_eq!(role_of("Compat", &l, false), NodeRoleDto::Other);
        assert_eq!(r("Textures/Things/A.png"), NodeRoleDto::Textures);
        assert_eq!(r("Sounds/Weapons"), NodeRoleDto::Sounds);
        assert_eq!(r("Source/Art/a.psd"), NodeRoleDto::Source);
        assert_eq!(r("Raw Assets/a.psd"), NodeRoleDto::Source);
        assert_eq!(r("Languages/English/Keyed/A.xml"), NodeRoleDto::Languages);
        assert_eq!(r("Assemblies/A.dll"), NodeRoleDto::Assemblies);
        assert_eq!(r("README.md"), NodeRoleDto::Other);
    }

    #[test]
    fn version_folders_and_common_hold_the_content() {
        let l = ProjectLayout::with_version_folder("1.6");
        assert_eq!(role_of("1.6", &l, false), NodeRoleDto::ContentRoot);
        assert_eq!(role_of("Common", &l, false), NodeRoleDto::ContentRoot);
        assert_eq!(
            role_of("1.5/Defs/ThingDefs_Misc/Weapons/A.xml", &l, false),
            NodeRoleDto::DefsWeapons
        );
        assert_eq!(
            role_of("1.6/Compat/CombatExtended/Patches/a.xml", &l, false),
            NodeRoleDto::CeCompat
        );
        assert_eq!(role_of("1.6NotOdyssey", &l, false), NodeRoleDto::Other);
        assert!(in_active_content("1.6/Defs/A.xml", &l));
        assert!(!in_active_content("1.5/Defs/A.xml", &l));
        assert!(in_active_content("Defs/A.xml", &ProjectLayout::default()));
    }

    #[test]
    fn a_legacy_ce_folder_and_a_flat_weapon_folder_are_recognised() {
        let l = ProjectLayout {
            ce_folder: "CE".into(),
            profile: LayoutProfile::Flat,
            weapons_dir: "ThingsDef_Misc/Weapons".into(),
            ..ProjectLayout::default()
        };
        assert_eq!(role_of("CE/Patches/a.xml", &l, true), NodeRoleDto::CeCompat);
        assert_eq!(
            role_of("Defs/ThingsDef_Misc/Weapons/a.xml", &l, true),
            NodeRoleDto::DefsWeapons
        );
        let bare = ProjectLayout {
            weapons_dir: String::new(),
            ..ProjectLayout::default()
        };
        assert_eq!(role_of("Defs/a.xml", &bare, false), NodeRoleDto::Defs);
    }

    #[test]
    fn facts_find_weapons_projectiles_and_ce_classes() {
        let xml = br#"<Defs>
          <ThingDef ParentName="BaseBullet"><defName>B</defName><projectile/></ThingDef>
          <ThingDef ParentName="BaseGun"><defName>G</defName><techLevel>Industrial</techLevel>
            <graphicData><texPath>Things/Item/Equipment/WeaponRanged/G</texPath></graphicData>
            <verbs><li><verbClass>Verb_Shoot</verbClass></li></verbs></ThingDef>
          <ThingDef ParentName="BaseMeleeWeapon"><defName>M</defName><tools><li/></tools></ThingDef>
          <ThingDef Name="Base" Abstract="True"><defName>X</defName><weaponTags/></ThingDef>
          <ThingDef><defName>Animal</defName><race/><tools><li/></tools></ThingDef>
        </Defs>"#;
        let f = facts_of("Defs/a.xml", xml);
        assert_eq!(f.root_tag.as_deref(), Some("Defs"));
        assert_eq!(f.projectiles, 1);
        let names: Vec<_> = f.weapons.iter().map(|w| w.def_name.as_str()).collect();
        assert_eq!(names, ["G", "M"]);
        assert!(f.weapons[0].ranged && !f.weapons[1].ranged);
        assert_eq!(f.weapons[0].tech_level.as_deref(), Some("Industrial"));
        assert!(!f.ce_class);
        let ce = facts_of(
            "Patches/a.xml",
            b"<Patch><Operation Class=\"CombatExtended.X\"/></Patch>",
        );
        assert!(ce.ce_class);
        let bad = facts_of("Defs/b.xml", b"<Defs><ThingDef></Defs>");
        assert!(bad.unparsable.is_some());
    }
}
