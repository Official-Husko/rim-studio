//! Scaffolding: the plan for a new mod folder, as node trees.
//!
//! [`plan`] turns a [`ScaffoldSpec`] into a [`ScaffoldPlan`]: the folders and files of a new mod
//! skeleton. Files that are XML hold a [`Node`] tree (the template data is JSON, see
//! [`ScaffoldPlan::to_json`]); the text is produced only at [`ScaffoldPlan::render`] time by
//! `rimstudio-xml`. The plan is pure: nothing here touches the disk. Applying it (creating the
//! folders and files, refusing to overwrite, going through the guard and fence) is the caller's job
//! with `rimstudio-io`; [`ScaffoldPlan::conflicts`] tells which planned files already exist.
//!
//! What is generated (RimStudio mod layout v1, `docs/features/mod-layout.md`):
//!
//! - `About/About.xml` through `rimstudio_xml::about::create_about`, with the supported versions and a
//!   description placeholder when the user gave none;
//! - the standard folders, named like the game's own: `Defs/ThingDefs_Misc/Weapons` and `Defs/SoundDefs`,
//!   `Patches`, `Textures/Things/Item/Equipment/WeaponRanged`, `WeaponMelee` and `Textures/Things/Projectile`,
//!   `Sounds/Weapons`, each on by default and each switchable; optional `Languages/English/Keyed`,
//!   `Assemblies` and `Source/Art` (author material that is never shipped);
//! - with the versioned layout, one folder per supported version plus `Common`;
//! - `LoadFolders.xml` when the layout is versioned or the optional Combat Extended folder is chosen. That
//!   folder (`Compat/CombatExtended`, with its own `Patches`) is only ever listed in `LoadFolders.xml` under an
//!   `IfModActive` condition for the Combat Extended package, so it never loads without that mod (owner
//!   rule: Combat Extended is an opt in patch, never part of the vanilla definitions);
//! - only when asked: `.gitignore` (and its Source art lines), `README.md`, `Credits.txt`, `.gitkeep` files.
//!   `About/Preview.png` and `About/Manifest.xml` are never created here.
//!
//! An invalid spec (empty name, a package id the game would warn about, no valid supported
//! version, a relative target) gives a plan with no items and [`codes::SCAFFOLD_INVALID`] problems.

use std::collections::BTreeMap;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::ids::PackageId;
use rimstudio_core::load_plan::{LoadEntry, LoadFoldersSpec};
use rimstudio_core::mods::ModDependency;
use rimstudio_core::paths;
use rimstudio_core::tree::Node;
use rimstudio_core::version::parse_major_minor;
use rimstudio_xml::about::{AboutSpec, create_about};
use rimstudio_xml::load_folders;
use rimstudio_xml::render::{RenderOpts, render};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::codes;

/// Where the content folders go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScaffoldLayout {
    /// `Defs`, `Patches` and the rest directly in the mod root.
    #[default]
    Flat,
    /// One folder per supported version (`1.6/Defs`, ...) plus `Common`, selected by
    /// `LoadFolders.xml`.
    Versioned,
}

/// What a new mod should contain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaffoldSpec {
    /// The folder to create (the mod root). Must be absolute.
    pub target: Utf8PathBuf,
    /// The display name.
    pub name: String,
    /// The package id (the game's format: ASCII letters, digits and dots, at least one dot).
    pub package_id: String,
    /// The author.
    pub author: String,
    /// The description text.
    pub description: String,
    /// The supported game versions, `major.minor` each.
    pub supported_versions: Vec<String>,
    /// Flat or versioned folders.
    pub layout: ScaffoldLayout,
    /// Create the definition folders: `Defs/ThingDefs_Misc/Weapons` and `Defs/SoundDefs`.
    pub defs_folder: bool,
    /// Create the `Patches` folder.
    pub patches_folder: bool,
    /// Create the texture folders of weapons and projectiles, named like the game's own.
    pub textures_folder: bool,
    /// Create `Sounds/Weapons`.
    pub sounds_folder: bool,
    /// Create `Source/Art` in the mod root: art sources and code, never shipped.
    pub source_folder: bool,
    /// Create `Languages/English/Keyed`.
    pub languages_folder: bool,
    /// Create the `Assemblies` folder.
    pub assemblies_folder: bool,
    /// Create the optional `Compat/CombatExtended/Patches` folder, gated in `LoadFolders.xml`. Off unless
    /// the user asks.
    pub ce_patch_folder: bool,
    /// Mods this mod depends on (written to About.xml, and as `loadAfter`).
    pub dependencies: Vec<ModDependency>,
    /// Put a `.gitkeep` file in every folder that would be empty.
    pub placeholder_files: bool,
    /// Write a `.gitignore` for a mod repository (operating system files, build output). Off unless asked.
    pub gitignore: bool,
    /// With `gitignore`: also keep `Source/Art` and `Raw Assets` out of the repository.
    pub ignore_source_art: bool,
    /// Write a `README.md` with the name and the description. Off unless asked.
    pub readme: bool,
    /// Write a `Credits.txt` naming the author. Off unless asked.
    pub credits: bool,
}

impl ScaffoldSpec {
    /// A spec with the usual defaults: version 1.6, flat layout, the definition, patch, texture and sound
    /// folders, no Combat Extended folder, no optional files, no placeholders.
    #[must_use]
    pub fn new(
        target: impl Into<Utf8PathBuf>,
        name: impl Into<String>,
        package_id: impl Into<String>,
    ) -> Self {
        ScaffoldSpec {
            target: target.into(),
            name: name.into(),
            package_id: package_id.into(),
            author: String::new(),
            description: String::new(),
            supported_versions: vec!["1.6".to_owned()],
            layout: ScaffoldLayout::Flat,
            defs_folder: true,
            patches_folder: true,
            textures_folder: true,
            sounds_folder: true,
            source_folder: false,
            languages_folder: false,
            assemblies_folder: false,
            ce_patch_folder: false,
            dependencies: Vec::new(),
            placeholder_files: false,
            gitignore: false,
            ignore_source_art: false,
            readme: false,
            credits: false,
        }
    }
}

/// The content of a planned file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum PlanContent {
    /// An XML file, as a node tree rendered by `rimstudio-xml`.
    Xml {
        /// The document element.
        node: Node,
    },
    /// A plain text file.
    Text {
        /// The text.
        text: String,
    },
}

/// One folder or file of a plan; paths are relative to the plan root, `/` separated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PlanItem {
    /// A folder to create.
    Dir {
        /// The relative path.
        path: String,
    },
    /// A file to create.
    File {
        /// The relative path.
        path: String,
        /// The content.
        content: PlanContent,
    },
}

impl PlanItem {
    /// The relative path of the item.
    #[must_use]
    pub fn path(&self) -> &str {
        match self {
            PlanItem::Dir { path } | PlanItem::File { path, .. } => path,
        }
    }
}

/// A file ready to be written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedFile {
    /// The relative path.
    pub path: String,
    /// The text to write.
    pub text: String,
}

/// The plan for a new mod folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScaffoldPlan {
    /// The folder the plan creates.
    pub root: Utf8PathBuf,
    /// Folders and files sorted by path (a folder before its files).
    pub items: Vec<PlanItem>,
    /// Problems with the spec; a plan with an error problem has no items.
    pub problems: Vec<Diagnostic>,
}

impl ScaffoldPlan {
    /// True when the spec was valid (no error problem).
    #[must_use]
    pub fn is_valid(&self) -> bool {
        !self.problems.iter().any(|p| p.severity == Severity::Error)
    }

    /// The planned files.
    pub fn files(&self) -> impl Iterator<Item = (&str, &PlanContent)> {
        self.items.iter().filter_map(|i| match i {
            PlanItem::File { path, content } => Some((path.as_str(), content)),
            PlanItem::Dir { .. } => None,
        })
    }

    /// The planned folders.
    pub fn dirs(&self) -> impl Iterator<Item = &str> {
        self.items.iter().filter_map(|i| match i {
            PlanItem::Dir { path } => Some(path.as_str()),
            PlanItem::File { .. } => None,
        })
    }

    /// Every file as text, XML rendered with `opts`. Deterministic.
    #[must_use]
    pub fn render(&self, opts: &RenderOpts) -> Vec<RenderedFile> {
        self.files()
            .map(|(path, content)| RenderedFile {
                path: path.to_owned(),
                text: match content {
                    PlanContent::Xml { node } => render(node, opts),
                    PlanContent::Text { text } => text.clone(),
                },
            })
            .collect()
    }

    /// The absolute path of an item.
    #[must_use]
    pub fn absolute(&self, relative: &str) -> Utf8PathBuf {
        self.root.join(relative)
    }

    /// The planned files for which `exists` answers true (given the absolute path). Applying a plan
    /// must refuse while this is not empty: a scaffold never overwrites.
    pub fn conflicts(&self, exists: &dyn Fn(&Utf8Path) -> bool) -> Vec<Utf8PathBuf> {
        self.files()
            .map(|(p, _)| self.absolute(p))
            .filter(|p| exists(p))
            .collect()
    }

    /// The plan as JSON (the node trees are the template data).
    #[must_use]
    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }
}

fn problem(message: impl Into<String>, field: &str) -> Diagnostic {
    Diagnostic::new(codes::SCAFFOLD_INVALID, Severity::Error, message).with_arg("field", field)
}

fn check(spec: &ScaffoldSpec) -> (Vec<Diagnostic>, Vec<String>) {
    let mut problems = Vec::new();
    if !spec.target.is_absolute() {
        problems.push(problem(
            "the target folder must be an absolute path",
            "target",
        ));
    }
    if spec.name.trim().is_empty() {
        problems.push(problem("the mod name is empty", "name"));
    }
    match PackageId::parse(&spec.package_id) {
        Ok(id) => {
            if let Err(why) = id.check_game_format() {
                problems.push(problem(
                    format!(
                        "the package id {:?} is not in the game's format ({why:?})",
                        spec.package_id
                    ),
                    "packageId",
                ));
            }
        }
        Err(e) => problems.push(problem(e.to_string(), "packageId")),
    }
    let mut versions: Vec<String> = Vec::new();
    for v in &spec.supported_versions {
        match parse_major_minor(v, true) {
            Some((major, minor)) => {
                let canonical = format!("{major}.{minor}");
                if !versions.contains(&canonical) {
                    versions.push(canonical);
                }
            }
            None => problems.push(problem(
                format!("{v:?} is not a game version (major.minor)"),
                "supportedVersions",
            )),
        }
    }
    if versions.is_empty()
        && !spec
            .supported_versions
            .iter()
            .any(|v| parse_major_minor(v, true).is_none())
    {
        problems.push(problem(
            "at least one supported version is needed",
            "supportedVersions",
        ));
    }
    (problems, versions)
}

/// The text of `About.xml`'s description when the user gave none.
pub const DESCRIPTION_PLACEHOLDER: &str = "Describe what this mod adds. This text is shown in the mod list and on the Steam Workshop page.";

fn gitignore_text(ignore_source_art: bool) -> String {
    let mut lines = vec![
        "# Operating system and editor files",
        ".DS_Store",
        "Thumbs.db",
        "*.bak",
        "*.tmp",
        "",
        "# Build output of the Source folder",
        "Source/**/bin/",
        "Source/**/obj/",
    ];
    if ignore_source_art {
        lines.extend([
            "",
            "# Art sources are not part of the mod",
            "Source/Art/",
            "Raw Assets/",
        ]);
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}

/// Plans a new mod folder.
#[must_use]
pub fn plan(spec: &ScaffoldSpec) -> ScaffoldPlan {
    let (problems, versions) = check(spec);
    if problems.iter().any(|p| p.severity == Severity::Error) {
        return ScaffoldPlan {
            root: spec.target.clone(),
            items: Vec::new(),
            problems,
        };
    }

    let mut items: BTreeMap<String, PlanItem> = BTreeMap::new();
    let mut leaf_dirs: Vec<String> = Vec::new();
    // a folder and every folder above it
    let mut put_path = |items: &mut BTreeMap<String, PlanItem>, path: &str, leaf: bool| {
        let mut acc = String::new();
        for part in path.split('/') {
            if !acc.is_empty() {
                acc.push('/');
            }
            acc.push_str(part);
            items.insert(acc.clone(), PlanItem::Dir { path: acc.clone() });
        }
        if leaf {
            leaf_dirs.push(path.to_owned());
        }
    };

    // content folders
    let bases: Vec<String> = match spec.layout {
        ScaffoldLayout::Flat => vec![String::new()],
        ScaffoldLayout::Versioned => versions.clone(),
    };
    let join = |base: &str, name: &str| {
        if base.is_empty() {
            name.to_owned()
        } else {
            format!("{base}/{name}")
        }
    };
    if spec.layout == ScaffoldLayout::Versioned {
        put_path(&mut items, paths::COMMON_DIR, true);
    }
    for base in &bases {
        if spec.layout == ScaffoldLayout::Versioned {
            put_path(&mut items, base, false);
        }
        let mut leaves: Vec<String> = Vec::new();
        if spec.defs_folder {
            leaves.push(join(
                base,
                &format!("{}/{}", paths::DEFS_DIR, paths::DEFS_WEAPONS_DIR),
            ));
            leaves.push(join(
                base,
                &format!("{}/{}", paths::DEFS_DIR, paths::DEFS_SOUNDS_DIR),
            ));
        }
        if spec.patches_folder {
            leaves.push(join(base, paths::PATCHES_DIR));
        }
        if spec.textures_folder {
            for dir in [
                paths::TEXTURES_WEAPON_RANGED_DIR,
                paths::TEXTURES_WEAPON_MELEE_DIR,
                paths::TEXTURES_PROJECTILE_DIR,
            ] {
                leaves.push(join(base, &format!("{}/{dir}", paths::TEXTURES_DIR)));
            }
        }
        if spec.sounds_folder {
            leaves.push(join(
                base,
                &format!("{}/{}", paths::SOUNDS_DIR, paths::SOUNDS_WEAPONS_DIR),
            ));
        }
        if spec.languages_folder {
            leaves.push(join(
                base,
                &format!("{}/{}", paths::LANGUAGES_DIR, paths::LANGUAGES_KEYED_DIR),
            ));
        }
        if spec.assemblies_folder {
            leaves.push(join(base, paths::ASSEMBLIES_DIR));
        }
        if spec.ce_patch_folder {
            leaves.push(join(
                base,
                &format!("{}/{}", paths::CE_COMPAT_DIR, paths::PATCHES_DIR),
            ));
        }
        for leaf in leaves {
            put_path(&mut items, &leaf, true);
        }
    }
    if spec.source_folder {
        put_path(&mut items, paths::SOURCE_ART_DIR, true);
    }

    // About.xml
    let mut load_after: Vec<String> = Vec::new();
    for dep in &spec.dependencies {
        let id = dep.package_id.trim();
        if !id.is_empty() && !load_after.iter().any(|l| l.eq_ignore_ascii_case(id)) {
            load_after.push(id.to_owned());
        }
    }
    let description = if spec.description.trim().is_empty() {
        DESCRIPTION_PLACEHOLDER.to_owned()
    } else {
        spec.description.clone()
    };
    let about = AboutSpec {
        name: spec.name.trim().to_owned(),
        author: spec.author.trim().to_owned(),
        package_id: spec.package_id.trim().to_owned(),
        description: description.clone(),
        supported_versions: versions.clone(),
        url: None,
        mod_version: None,
        mod_dependencies: spec.dependencies.clone(),
        load_after,
        load_before: Vec::new(),
        incompatible_with: Vec::new(),
    };
    let about_path = paths::ABOUT_XML.to_owned();
    put_path(&mut items, paths::ABOUT_DIR, false);
    items.insert(
        about_path.clone(),
        PlanItem::File {
            path: about_path,
            content: PlanContent::Xml {
                node: create_about(&about),
            },
        },
    );

    // LoadFolders.xml
    if spec.layout == ScaffoldLayout::Versioned || spec.ce_patch_folder {
        let mut lf = LoadFoldersSpec::new();
        for v in &versions {
            let mut entries: Vec<LoadEntry> = match spec.layout {
                ScaffoldLayout::Versioned => {
                    vec![LoadEntry::dir(paths::COMMON_DIR), LoadEntry::dir(v.clone())]
                }
                ScaffoldLayout::Flat => vec![LoadEntry::root()],
            };
            if spec.ce_patch_folder {
                let base = match spec.layout {
                    ScaffoldLayout::Versioned => format!("{v}/"),
                    ScaffoldLayout::Flat => String::new(),
                };
                entries.push(
                    LoadEntry::dir(format!("{base}{}", paths::CE_COMPAT_DIR))
                        .if_active([paths::CE_PACKAGE_ID]),
                );
            }
            lf.add_block(v, entries);
        }
        let path = paths::LOAD_FOLDERS_XML.to_owned();
        items.insert(
            path.clone(),
            PlanItem::File {
                path,
                content: PlanContent::Xml {
                    node: load_folders::to_node(&lf),
                },
            },
        );
    }

    // optional files: only when asked
    let put_text = |items: &mut BTreeMap<String, PlanItem>, path: &str, text: String| {
        items.insert(
            path.to_owned(),
            PlanItem::File {
                path: path.to_owned(),
                content: PlanContent::Text { text },
            },
        );
    };
    if spec.gitignore {
        put_text(
            &mut items,
            ".gitignore",
            gitignore_text(spec.ignore_source_art),
        );
    }
    if spec.readme {
        put_text(
            &mut items,
            "README.md",
            format!("# {}\n\n{}\n", spec.name.trim(), description.trim()),
        );
    }
    if spec.credits {
        let author = spec.author.trim();
        put_text(
            &mut items,
            "Credits.txt",
            format!(
                "{}\n\nAuthor: {}\n",
                spec.name.trim(),
                if author.is_empty() {
                    "(your name)"
                } else {
                    author
                }
            ),
        );
    }

    // placeholders in folders that stay empty
    if spec.placeholder_files {
        for dir in leaf_dirs {
            let path = format!("{dir}/.gitkeep");
            items.insert(
                path.clone(),
                PlanItem::File {
                    path,
                    content: PlanContent::Text {
                        text: String::new(),
                    },
                },
            );
        }
    }

    ScaffoldPlan {
        root: spec.target.clone(),
        items: items.into_values().collect(),
        problems,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_xml::about::read_lenient;
    use rimstudio_xml::{ParseMode, parse_document};

    fn spec() -> ScaffoldSpec {
        let mut s = ScaffoldSpec::new("/mods/RS_NewMod", "RS New Mod", "rs.newmod");
        s.author = "RS Tester".into();
        s.description = "A fictional mod.".into();
        s
    }

    fn paths_of(plan: &ScaffoldPlan) -> Vec<String> {
        plan.items.iter().map(|i| i.path().to_owned()).collect()
    }

    fn text_of(plan: &ScaffoldPlan, path: &str) -> String {
        plan.render(&RenderOpts::default())
            .into_iter()
            .find(|f| f.path == path)
            .map(|f| f.text)
            .unwrap_or_default()
    }

    #[test]
    fn flat_plan_is_the_standard_skeleton() {
        let plan = plan(&spec());
        assert!(plan.is_valid());
        assert_eq!(
            paths_of(&plan),
            vec![
                "About",
                "About/About.xml",
                "Defs",
                "Defs/SoundDefs",
                "Defs/ThingDefs_Misc",
                "Defs/ThingDefs_Misc/Weapons",
                "Patches",
                "Sounds",
                "Sounds/Weapons",
                "Textures",
                "Textures/Things",
                "Textures/Things/Item",
                "Textures/Things/Item/Equipment",
                "Textures/Things/Item/Equipment/WeaponMelee",
                "Textures/Things/Item/Equipment/WeaponRanged",
                "Textures/Things/Projectile",
            ]
        );
        assert!(plan.files().all(|(p, _)| p == "About/About.xml"));
        assert!(text_of(&plan, "About/About.xml").contains("<packageId>rs.newmod</packageId>"));
    }

    #[test]
    fn every_standard_folder_can_be_switched_off() {
        let mut s = spec();
        s.defs_folder = false;
        s.textures_folder = false;
        s.sounds_folder = false;
        s.patches_folder = false;
        assert_eq!(paths_of(&plan(&s)), vec!["About", "About/About.xml"]);
    }

    #[test]
    fn optional_folders_and_files_appear_only_when_asked() {
        let mut s = spec();
        s.languages_folder = true;
        s.assemblies_folder = true;
        s.source_folder = true;
        s.gitignore = true;
        s.ignore_source_art = true;
        s.readme = true;
        s.credits = true;
        let plan = plan(&s);
        let p = paths_of(&plan);
        for expected in [
            "Languages/English/Keyed",
            "Assemblies",
            "Source/Art",
            ".gitignore",
            "README.md",
            "Credits.txt",
        ] {
            assert!(p.contains(&expected.to_owned()), "{expected}");
        }
        let ignore = text_of(&plan, ".gitignore");
        assert!(ignore.contains("Source/Art/") && ignore.contains("Raw Assets/"));
        assert!(text_of(&plan, "Credits.txt").contains("Author: RS Tester"));
        assert!(text_of(&plan, "README.md").starts_with("# RS New Mod"));
        // never created silently
        let base = super::plan(&spec());
        for never in [
            "About/Preview.png",
            "About/Manifest.xml",
            "README.md",
            ".gitignore",
        ] {
            assert!(!paths_of(&base).contains(&never.to_owned()), "{never}");
        }
        let plain = ScaffoldSpec {
            gitignore: true,
            ..spec()
        };
        assert!(!text_of(&super::plan(&plain), ".gitignore").contains("Source/Art/"));
    }

    #[test]
    fn an_empty_description_gets_the_placeholder() {
        let mut s = spec();
        s.description = "  ".into();
        let text = text_of(&plan(&s), "About/About.xml");
        assert!(text.contains(DESCRIPTION_PLACEHOLDER));
    }

    #[test]
    fn the_generated_about_reads_back_without_warnings() {
        let plan = plan(&spec());
        let text = text_of(&plan, "About/About.xml");
        let read = read_lenient(text.as_bytes());
        assert!(read.parsed);
        assert!(read.warnings.is_empty(), "{:?}", read.warnings);
        assert_eq!(read.about.package_id, "rs.newmod");
        assert_eq!(read.about.supported_versions, vec!["1.6".to_owned()]);
        assert_eq!(read.about.name, "RS New Mod");
    }

    #[test]
    fn versioned_layout_writes_load_folders_per_version() {
        let mut s = spec();
        s.layout = ScaffoldLayout::Versioned;
        s.supported_versions = vec!["1.5".into(), "v1.6".into(), "1.6".into()];
        s.defs_folder = false;
        s.textures_folder = false;
        s.sounds_folder = false;
        s.languages_folder = true;
        let plan = plan(&s);
        assert_eq!(
            paths_of(&plan),
            vec![
                "1.5",
                "1.5/Languages",
                "1.5/Languages/English",
                "1.5/Languages/English/Keyed",
                "1.5/Patches",
                "1.6",
                "1.6/Languages",
                "1.6/Languages/English",
                "1.6/Languages/English/Keyed",
                "1.6/Patches",
                "About",
                "About/About.xml",
                "Common",
                "LoadFolders.xml"
            ]
        );
        let lf = text_of(&plan, "LoadFolders.xml");
        let parsed = load_folders::read(lf.as_bytes()).unwrap();
        assert_eq!(parsed.spec.blocks.len(), 2);
        let b = parsed.spec.block("1.6").unwrap();
        assert_eq!(
            b.entries
                .iter()
                .map(|e| e.path.as_str())
                .collect::<Vec<_>>(),
            vec!["Common", "1.6"]
        );
    }

    #[test]
    fn the_combat_extended_folder_is_opt_in_and_gated() {
        let off = plan(&spec());
        assert!(!paths_of(&off).iter().any(|p| p.contains("CombatExtended")));
        assert!(!paths_of(&off).contains(&"LoadFolders.xml".to_owned()));

        let mut s = spec();
        s.ce_patch_folder = true;
        let on = plan(&s);
        let p = paths_of(&on);
        assert!(p.contains(&"Compat/CombatExtended/Patches".to_owned()));
        let lf = load_folders::read(text_of(&on, "LoadFolders.xml").as_bytes()).unwrap();
        let block = lf.spec.block("1.6").unwrap();
        let gated = block
            .entries
            .iter()
            .find(|e| e.path == "Compat/CombatExtended")
            .unwrap();
        assert_eq!(gated.if_active, vec!["ceteam.combatextended".to_owned()]);
        let root = block.entries.iter().find(|e| e.path.is_empty()).unwrap();
        assert!(!root.is_conditional());
        // no file outside the CE folder mentions a Combat Extended class
        for f in on.render(&RenderOpts::default()) {
            assert!(!f.text.contains("CombatExtended."), "{}", f.path);
            assert!(!f.text.contains("ceteam") || f.path == "LoadFolders.xml");
        }
    }

    #[test]
    fn placeholders_only_when_asked() {
        let mut s = spec();
        s.placeholder_files = true;
        let plan = plan(&s);
        assert!(paths_of(&plan).contains(&"Defs/ThingDefs_Misc/Weapons/.gitkeep".to_owned()));
        assert!(paths_of(&plan).contains(&"Patches/.gitkeep".to_owned()));
        assert!(!paths_of(&plan).contains(&"Defs/.gitkeep".to_owned()));
        let none = super::plan(&spec());
        assert!(!paths_of(&none).iter().any(|p| p.ends_with(".gitkeep")));
    }

    #[test]
    fn dependencies_become_dependencies_and_load_after() {
        let mut s = spec();
        s.dependencies.push(ModDependency {
            package_id: "rs.base".into(),
            display_name: "RS Base".into(),
            ..ModDependency::default()
        });
        let plan = plan(&s);
        let read = read_lenient(text_of(&plan, "About/About.xml").as_bytes());
        assert_eq!(read.about.mod_dependencies.len(), 1);
        assert_eq!(read.about.load_after, vec!["rs.base".to_owned()]);
    }

    #[test]
    fn invalid_specs_give_no_items_and_named_problems() {
        let cases: Vec<(ScaffoldSpec, &str)> = vec![
            (ScaffoldSpec::new("/m/x", "  ", "rs.x"), "name"),
            (ScaffoldSpec::new("/m/x", "X", "nodots"), "packageId"),
            (ScaffoldSpec::new("/m/x", "X", "rs..x"), "packageId"),
            (ScaffoldSpec::new("/m/x", "X", ""), "packageId"),
            (ScaffoldSpec::new("relative/x", "X", "rs.x"), "target"),
            (
                {
                    let mut s = ScaffoldSpec::new("/m/x", "X", "rs.x");
                    s.supported_versions = vec!["abc".into()];
                    s
                },
                "supportedVersions",
            ),
            (
                {
                    let mut s = ScaffoldSpec::new("/m/x", "X", "rs.x");
                    s.supported_versions = Vec::new();
                    s
                },
                "supportedVersions",
            ),
        ];
        for (spec, field) in cases {
            let p = plan(&spec);
            assert!(!p.is_valid(), "{spec:?}");
            assert!(p.items.is_empty());
            assert!(
                p.problems
                    .iter()
                    .any(|d| d.args.get("field").map(String::as_str) == Some(field)),
                "{field}: {:?}",
                p.problems
            );
        }
    }

    #[test]
    fn conflicts_list_existing_planned_files() {
        let p = plan(&spec());
        let existing = Utf8PathBuf::from("/mods/RS_NewMod/About/About.xml");
        let hits = p.conflicts(&|path| path == existing);
        assert_eq!(hits, vec![existing]);
        assert!(p.conflicts(&|_| false).is_empty());
    }

    #[test]
    fn planning_is_deterministic_and_round_trips_through_json() {
        let mut s = spec();
        s.layout = ScaffoldLayout::Versioned;
        s.ce_patch_folder = true;
        let a = plan(&s);
        let b = plan(&s);
        assert_eq!(a, b);
        assert_eq!(a.to_json(), b.to_json());
        let back: ScaffoldPlan = serde_json::from_value(a.to_json()).unwrap();
        assert_eq!(back, a);
        for f in a.render(&RenderOpts::default()) {
            if f.path.ends_with(".xml") {
                assert!(
                    parse_document(f.text.as_bytes(), ParseMode::Game).is_ok(),
                    "{}",
                    f.path
                );
            }
        }
    }
}
