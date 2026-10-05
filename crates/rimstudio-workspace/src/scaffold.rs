//! Scaffolding: the plan for a new mod folder, as node trees.
//!
//! [`plan`] turns a [`ScaffoldSpec`] into a [`ScaffoldPlan`]: the folders and files of a new mod
//! skeleton. Files that are XML hold a [`Node`] tree (the template data is JSON, see
//! [`ScaffoldPlan::to_json`]); the text is produced only at [`ScaffoldPlan::render`] time by
//! `rimstudio-xml`. The plan is pure: nothing here touches the disk. Applying it (creating the
//! folders and files, refusing to overwrite, going through the guard and fence) is the caller's job
//! with `rimstudio-io`; [`ScaffoldPlan::conflicts`] tells which planned files already exist.
//!
//! What is generated:
//!
//! - `About/About.xml` through `rimstudio_xml::about::create_about`;
//! - empty `Defs`, `Patches`, `Languages` and `Assemblies` folders as chosen (no placeholder files
//!   unless asked);
//! - with the versioned layout, one folder per supported version plus `Common`;
//! - `LoadFolders.xml` when the layout is versioned or the optional Combat Extended patch folder is
//!   chosen. That folder is only ever listed in `LoadFolders.xml` under an `IfModActive` condition
//!   for the Combat Extended package, so it never loads without that mod (owner rule: Combat
//!   Extended is an opt in patch, never part of the vanilla definitions).
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
    /// Create the `Defs` folder.
    pub defs_folder: bool,
    /// Create the `Patches` folder.
    pub patches_folder: bool,
    /// Create the `Languages` folder.
    pub languages_folder: bool,
    /// Create the `Assemblies` folder.
    pub assemblies_folder: bool,
    /// Create the optional Combat Extended patch folder, gated in `LoadFolders.xml`. Off unless the
    /// user asks.
    pub ce_patch_folder: bool,
    /// Mods this mod depends on (written to About.xml, and as `loadAfter`).
    pub dependencies: Vec<ModDependency>,
    /// Put a `.gitkeep` file in every folder that would be empty.
    pub placeholder_files: bool,
}

impl ScaffoldSpec {
    /// A spec with the usual defaults: version 1.6, flat layout, `Defs` and `Patches` folders, no
    /// Combat Extended folder, no placeholders.
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
            languages_folder: false,
            assemblies_folder: false,
            ce_patch_folder: false,
            dependencies: Vec::new(),
            placeholder_files: false,
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
    let mut put_dir = |path: String| {
        items.insert(path.clone(), PlanItem::Dir { path });
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
    let mut leaf_dirs: Vec<String> = Vec::new();
    if spec.layout == ScaffoldLayout::Versioned {
        put_dir(paths::COMMON_DIR.to_owned());
        leaf_dirs.push(paths::COMMON_DIR.to_owned());
    }
    for base in &bases {
        if spec.layout == ScaffoldLayout::Versioned {
            put_dir(base.clone());
        }
        let mut subs: Vec<&str> = Vec::new();
        if spec.defs_folder {
            subs.push(paths::DEFS_DIR);
        }
        if spec.patches_folder {
            subs.push(paths::PATCHES_DIR);
        }
        if spec.languages_folder {
            subs.push(paths::LANGUAGES_DIR);
        }
        if spec.assemblies_folder {
            subs.push(paths::ASSEMBLIES_DIR);
        }
        for sub in subs {
            let path = join(base, sub);
            put_dir(path.clone());
            leaf_dirs.push(path);
        }
        if spec.ce_patch_folder {
            let ce = join(base, paths::CE_PATCH_FOLDER_NAME);
            let patches = format!("{ce}/{}", paths::PATCHES_DIR);
            put_dir(ce);
            put_dir(patches.clone());
            leaf_dirs.push(patches);
        }
    }

    // About.xml
    let mut load_after: Vec<String> = Vec::new();
    for dep in &spec.dependencies {
        let id = dep.package_id.trim();
        if !id.is_empty() && !load_after.iter().any(|l| l.eq_ignore_ascii_case(id)) {
            load_after.push(id.to_owned());
        }
    }
    let about = AboutSpec {
        name: spec.name.trim().to_owned(),
        author: spec.author.trim().to_owned(),
        package_id: spec.package_id.trim().to_owned(),
        description: spec.description.clone(),
        supported_versions: versions.clone(),
        url: None,
        mod_version: None,
        mod_dependencies: spec.dependencies.clone(),
        load_after,
        load_before: Vec::new(),
        incompatible_with: Vec::new(),
    };
    let about_path = paths::ABOUT_XML.to_owned();
    put_dir(paths::ABOUT_DIR.to_owned());
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
                    LoadEntry::dir(format!("{base}{}", paths::CE_PATCH_FOLDER_NAME))
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
    fn flat_plan_has_about_and_empty_folders_only() {
        let plan = plan(&spec());
        assert!(plan.is_valid());
        assert_eq!(
            paths_of(&plan),
            vec!["About", "About/About.xml", "Defs", "Patches"]
        );
        assert!(plan.files().all(|(p, _)| p == "About/About.xml"));
        assert!(text_of(&plan, "About/About.xml").contains("<packageId>rs.newmod</packageId>"));
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
        s.languages_folder = true;
        let plan = plan(&s);
        assert_eq!(
            paths_of(&plan),
            vec![
                "1.5",
                "1.5/Defs",
                "1.5/Languages",
                "1.5/Patches",
                "1.6",
                "1.6/Defs",
                "1.6/Languages",
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
        assert!(p.contains(&"CombatExtended/Patches".to_owned()));
        let lf = load_folders::read(text_of(&on, "LoadFolders.xml").as_bytes()).unwrap();
        let block = lf.spec.block("1.6").unwrap();
        let gated = block
            .entries
            .iter()
            .find(|e| e.path == "CombatExtended")
            .unwrap();
        assert_eq!(gated.if_active, vec!["ceteam.combatextended".to_owned()]);
        let root = block.entries.iter().find(|e| e.path.is_empty()).unwrap();
        assert!(!root.is_conditional());
        // no file outside the CE folder mentions a Combat Extended class
        for f in on.render(&RenderOpts::default()) {
            assert!(!f.text.contains("CombatExtended."), "{}", f.path);
        }
    }

    #[test]
    fn placeholders_only_when_asked() {
        let mut s = spec();
        s.placeholder_files = true;
        let plan = plan(&s);
        assert!(paths_of(&plan).contains(&"Defs/.gitkeep".to_owned()));
        assert!(paths_of(&plan).contains(&"Patches/.gitkeep".to_owned()));
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
