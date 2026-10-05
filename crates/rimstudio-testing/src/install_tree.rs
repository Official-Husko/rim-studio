//! Fictional RimWorld installs and mod folders on disk.
//!
//! [`InstallBuilder`] writes a miniature install into a directory (or a temporary one): a game
//! folder with `Version.txt`, `Data/Core` and optional expansion folders, an install `Mods`
//! folder, a Steam style Workshop content folder, a custom mod folder and a user data folder with
//! `ModsConfig.xml`. [`ModFolder`] describes one mod: `About.xml` (with the quirks seen in real
//! mods), definition and patch files, `LoadFolders.xml`, arbitrary extra files. Every XML file is
//! built from `rimstudio_core::tree` nodes and rendered with `rimstudio-xml`.
//!
//! ```
//! use rimstudio_core::tree::NodeBuilder;
//! use rimstudio_testing::install_tree::{InstallBuilder, ModFolder};
//!
//! let def = NodeBuilder::new("ThingDef").text_elem("defName", "RS_TestRifle").build();
//! let install = InstallBuilder::new()
//!     .core_def(def.clone())
//!     .mod_folder(ModFolder::new("RS_TestMod", "rs.testmod").def(def))
//!     .build_temp()
//!     .unwrap();
//! assert!(install.mods_dir.join("RS_TestMod/About/About.xml").is_file());
//! ```
//!
//! Everything is fictional (names start with `RS_`); no game data is embedded. Test code may
//! panic, so builders do not return typed errors: writing returns `std::io::Error`.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::ops::Deref;
use std::path::Path;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::load_plan::LoadFoldersSpec;
use rimstudio_core::mods::{ModDependency, VersionRelations};
use rimstudio_core::tree::{Child, Node, NodeBuilder};
use rimstudio_xml::about::{AboutSpec, create_about};
use rimstudio_xml::load_folders;
use rimstudio_xml::mods_config::{self, ModsConfigData};
use rimstudio_xml::render::{LineEnding, RenderOpts, Section, render, render_list_file};

/// The game version written to `Version.txt` unless the builder is told otherwise (fictional).
pub const DEFAULT_GAME_VERSION: &str = "1.6.1000 rev100";

/// Oddities of real mods that a fixture can reproduce.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Quirks {
    /// Name the metadata folder `about` instead of `About` (the game does not find it on a case
    /// sensitive file system).
    pub lowercase_about_dir: bool,
    /// The metadata file name, for example `about.xml`. Default `About.xml`.
    pub about_file_name: Option<String>,
    /// The load folders file name, for example `loadFolders.xml`. Default `LoadFolders.xml`.
    pub load_folders_file_name: Option<String>,
    /// Write a UTF-8 byte order mark at the start of every generated XML file.
    pub bom: bool,
    /// Use CRLF line endings in every generated XML file.
    pub crlf: bool,
}

#[derive(Debug, Clone)]
enum About {
    Generated,
    Raw(Vec<u8>),
    Missing,
}

#[derive(Debug, Clone)]
enum Content {
    /// A list file: a root with the given tag holding these nodes.
    List {
        root_tag: &'static str,
        nodes: Vec<Node>,
    },
    Bytes(Vec<u8>),
}

/// One mod folder: metadata, content files and load folder rules.
#[derive(Debug, Clone)]
pub struct ModFolder {
    folder_name: String,
    package_id: String,
    spec: AboutSpec,
    by_version: BTreeMap<String, VersionRelations>,
    about: About,
    files: Vec<(String, Content)>,
    empty_dirs: Vec<String>,
    load_folders: Option<LoadFoldersSpec>,
    quirks: Quirks,
}

/// The default file that [`ModFolder::def`] collects definitions into.
pub const DEFAULT_DEFS_FILE: &str = "Defs/RS_Defs.xml";

impl ModFolder {
    /// A mod in folder `folder_name` with the given package id, supporting game version 1.6.
    #[must_use]
    pub fn new(folder_name: impl Into<String>, package_id: impl Into<String>) -> Self {
        let folder_name = folder_name.into();
        let package_id = package_id.into();
        let spec = AboutSpec {
            name: folder_name.clone(),
            author: "RS Tester".to_owned(),
            package_id: package_id.clone(),
            description: "A fictional mod for tests.".to_owned(),
            supported_versions: vec!["1.6".to_owned()],
            ..AboutSpec::default()
        };
        Self {
            folder_name,
            package_id,
            spec,
            by_version: BTreeMap::new(),
            about: About::Generated,
            files: Vec::new(),
            empty_dirs: Vec::new(),
            load_folders: None,
            quirks: Quirks::default(),
        }
    }

    /// The folder name.
    #[must_use]
    pub fn folder_name(&self) -> &str {
        &self.folder_name
    }

    /// The package id given to [`ModFolder::new`].
    #[must_use]
    pub fn package_id(&self) -> &str {
        &self.package_id
    }

    /// Sets the display name.
    #[must_use]
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.spec.name = name.into();
        self
    }

    /// Sets the author.
    #[must_use]
    pub fn author(mut self, author: impl Into<String>) -> Self {
        self.spec.author = author.into();
        self
    }

    /// Sets the description.
    #[must_use]
    pub fn description(mut self, text: impl Into<String>) -> Self {
        self.spec.description = text.into();
        self
    }

    /// Sets `supportedVersions`.
    #[must_use]
    pub fn supports(mut self, versions: &[&str]) -> Self {
        self.spec.supported_versions = versions.iter().map(|v| (*v).to_owned()).collect();
        self
    }

    /// Sets `loadAfter`.
    #[must_use]
    pub fn load_after(mut self, ids: &[&str]) -> Self {
        self.spec.load_after = ids.iter().map(|v| (*v).to_owned()).collect();
        self
    }

    /// Sets `loadBefore`.
    #[must_use]
    pub fn load_before(mut self, ids: &[&str]) -> Self {
        self.spec.load_before = ids.iter().map(|v| (*v).to_owned()).collect();
        self
    }

    /// Sets `incompatibleWith`.
    #[must_use]
    pub fn incompatible_with(mut self, ids: &[&str]) -> Self {
        self.spec.incompatible_with = ids.iter().map(|v| (*v).to_owned()).collect();
        self
    }

    /// Adds a `modDependencies` entry with a fictional download URL.
    #[must_use]
    pub fn depends_on(self, package_id: &str, display_name: &str) -> Self {
        self.dependency(ModDependency {
            package_id: package_id.to_owned(),
            display_name: display_name.to_owned(),
            download_url: Some("https://example.invalid/rs".to_owned()),
            ..ModDependency::default()
        })
    }

    /// Adds a full `modDependencies` entry.
    #[must_use]
    pub fn dependency(mut self, dep: ModDependency) -> Self {
        self.spec.mod_dependencies.push(dep);
        self
    }

    /// Adds a per version block (`loadAfterByVersion`, `modDependenciesByVersion` and the others,
    /// depending on which lists are filled). `version` is the key as written, for example `1.6`.
    #[must_use]
    pub fn by_version(mut self, version: impl Into<String>, relations: VersionRelations) -> Self {
        self.by_version.insert(version.into(), relations);
        self
    }

    /// Adds one definition to the default definitions file ([`DEFAULT_DEFS_FILE`]).
    #[must_use]
    pub fn def(self, node: impl Into<Node>) -> Self {
        self.add_node(DEFAULT_DEFS_FILE.to_owned(), "Defs", node.into())
    }

    /// Adds a definitions file `Defs/<file>` (the file may contain sub folders).
    #[must_use]
    pub fn defs_file(self, file: &str, nodes: Vec<Node>) -> Self {
        self.set_list(format!("Defs/{file}"), "Defs", nodes)
    }

    /// Adds a definitions file inside a version or common folder: `<dir>/Defs/<file>`.
    #[must_use]
    pub fn defs_file_in(self, dir: &str, file: &str, nodes: Vec<Node>) -> Self {
        self.set_list(format!("{dir}/Defs/{file}"), "Defs", nodes)
    }

    /// Adds a patch file `Patches/<file>` holding the given operations.
    #[must_use]
    pub fn patch_file(self, file: &str, operations: Vec<Node>) -> Self {
        self.set_list(format!("Patches/{file}"), "Patch", operations)
    }

    /// Adds a patch file inside a version or common folder: `<dir>/Patches/<file>`.
    #[must_use]
    pub fn patch_file_in(self, dir: &str, file: &str, operations: Vec<Node>) -> Self {
        self.set_list(format!("{dir}/Patches/{file}"), "Patch", operations)
    }

    /// Sets the `LoadFolders.xml` content.
    #[must_use]
    pub fn load_folders(mut self, spec: LoadFoldersSpec) -> Self {
        self.load_folders = Some(spec);
        self
    }

    /// Adds a file with raw bytes at a path relative to the mod root (for broken files).
    #[must_use]
    pub fn raw_file(mut self, rel_path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> Self {
        self.files
            .push((rel_path.into(), Content::Bytes(bytes.into())));
        self
    }

    /// Adds an empty folder at a path relative to the mod root.
    #[must_use]
    pub fn empty_dir(mut self, rel_path: impl Into<String>) -> Self {
        self.empty_dirs.push(rel_path.into());
        self
    }

    /// Replaces the generated `About.xml` with raw bytes.
    #[must_use]
    pub fn raw_about(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.about = About::Raw(bytes.into());
        self
    }

    /// Writes no `About.xml` at all.
    #[must_use]
    pub fn no_about(mut self) -> Self {
        self.about = About::Missing;
        self
    }

    /// Sets all quirks at once.
    #[must_use]
    pub fn quirks(mut self, quirks: Quirks) -> Self {
        self.quirks = quirks;
        self
    }

    /// Names the metadata folder `about` instead of `About`.
    #[must_use]
    pub fn lowercase_about_dir(mut self) -> Self {
        self.quirks.lowercase_about_dir = true;
        self
    }

    /// Names the metadata file `name` (for example `about.xml`).
    #[must_use]
    pub fn about_file_name(mut self, name: impl Into<String>) -> Self {
        self.quirks.about_file_name = Some(name.into());
        self
    }

    /// Names the load folders file `name` (for example `loadFolders.xml`).
    #[must_use]
    pub fn load_folders_file_name(mut self, name: impl Into<String>) -> Self {
        self.quirks.load_folders_file_name = Some(name.into());
        self
    }

    /// Writes a byte order mark at the start of every generated XML file.
    #[must_use]
    pub fn bom(mut self) -> Self {
        self.quirks.bom = true;
        self
    }

    /// Uses CRLF line endings in every generated XML file.
    #[must_use]
    pub fn crlf(mut self) -> Self {
        self.quirks.crlf = true;
        self
    }

    fn set_list(mut self, rel: String, root_tag: &'static str, nodes: Vec<Node>) -> Self {
        match self.files.iter_mut().find(|(p, _)| *p == rel) {
            Some(slot) => slot.1 = Content::List { root_tag, nodes },
            None => self.files.push((rel, Content::List { root_tag, nodes })),
        }
        self
    }

    fn add_node(mut self, rel: String, root_tag: &'static str, node: Node) -> Self {
        match self.files.iter_mut().find(|(p, _)| *p == rel) {
            Some((_, Content::List { nodes, .. })) => nodes.push(node),
            Some(slot) => {
                slot.1 = Content::List {
                    root_tag,
                    nodes: vec![node],
                }
            }
            None => self.files.push((
                rel,
                Content::List {
                    root_tag,
                    nodes: vec![node],
                },
            )),
        }
        self
    }

    fn opts(&self) -> RenderOpts {
        RenderOpts {
            bom: self.quirks.bom,
            eol: if self.quirks.crlf {
                LineEnding::Crlf
            } else {
                LineEnding::Lf
            },
            ..RenderOpts::default()
        }
    }

    /// The `About.xml` tree: the generated spec plus the per version blocks.
    #[must_use]
    pub fn about_node(&self) -> Node {
        let mut node = create_about(&self.spec);
        let mut extra: Vec<Node> = Vec::new();
        let mut blocks = |tag: &str, pick: &dyn Fn(&VersionRelations) -> Option<Node>| {
            let kids: Vec<Node> = self
                .by_version
                .iter()
                .filter_map(|(k, rel)| {
                    pick(rel).map(|mut n| {
                        n.tag = version_tag(k);
                        n
                    })
                })
                .collect();
            if !kids.is_empty() {
                extra.push(NodeBuilder::new(tag).children(kids).build());
            }
        };
        blocks("modDependenciesByVersion", &|r| {
            (!r.mod_dependencies.is_empty()).then(|| {
                NodeBuilder::new("x")
                    .children(r.mod_dependencies.iter().map(dependency_li))
                    .build()
            })
        });
        blocks("loadBeforeByVersion", &|r| list_block(&r.load_before));
        blocks("loadAfterByVersion", &|r| list_block(&r.load_after));
        blocks("forceLoadBeforeByVersion", &|r| {
            list_block(&r.force_load_before)
        });
        blocks("forceLoadAfterByVersion", &|r| {
            list_block(&r.force_load_after)
        });
        blocks("incompatibleWithByVersion", &|r| {
            list_block(&r.incompatible_with)
        });
        // Keep the description last, as authors usually write it.
        let at = node.children.len().saturating_sub(1);
        for (i, n) in extra.into_iter().enumerate() {
            node.children.insert(at + i, Child::Element(n));
        }
        node
    }

    /// Writes the mod into `parent/<folder name>` and returns the mod folder path.
    ///
    /// # Errors
    /// Any file system error.
    pub fn write_to(&self, parent: &Utf8Path) -> io::Result<Utf8PathBuf> {
        let root = parent.join(&self.folder_name);
        fs::create_dir_all(&root)?;
        let opts = self.opts();
        let about_dir = if self.quirks.lowercase_about_dir {
            "about"
        } else {
            "About"
        };
        let about_name = self
            .quirks
            .about_file_name
            .as_deref()
            .unwrap_or("About.xml");
        match &self.about {
            About::Generated => {
                write_bytes(
                    &root.join(about_dir).join(about_name),
                    render(&self.about_node(), &opts).as_bytes(),
                )?;
            }
            About::Raw(bytes) => write_bytes(&root.join(about_dir).join(about_name), bytes)?,
            About::Missing => {}
        }
        if let Some(spec) = &self.load_folders {
            let name = self
                .quirks
                .load_folders_file_name
                .as_deref()
                .unwrap_or("LoadFolders.xml");
            write_bytes(
                &root.join(name),
                load_folders::create(spec, &opts).as_bytes(),
            )?;
        }
        for (rel, content) in &self.files {
            let path = root.join(rel);
            match content {
                Content::Bytes(b) => write_bytes(&path, b)?,
                Content::List { root_tag, nodes } => {
                    let text = render_list_file(root_tag, &[Section::plain(nodes)], &opts);
                    write_bytes(&path, text.as_bytes())?;
                }
            }
        }
        for dir in &self.empty_dirs {
            fs::create_dir_all(root.join(dir))?;
        }
        Ok(root)
    }
}

fn version_tag(key: &str) -> String {
    if key.starts_with(|c: char| c.is_ascii_digit()) {
        format!("v{key}")
    } else {
        key.to_owned()
    }
}

fn list_block(items: &[String]) -> Option<Node> {
    (!items.is_empty()).then(|| NodeBuilder::new("x").li_each(items.iter().cloned()).build())
}

fn dependency_li(dep: &ModDependency) -> Node {
    NodeBuilder::new("li")
        .text_elem("packageId", dep.package_id.clone())
        .text_elem("displayName", dep.display_name.clone())
        .text_elem_opt("steamWorkshopUrl", dep.steam_workshop_url.clone())
        .text_elem_opt("downloadUrl", dep.download_url.clone())
        .build()
}

fn write_bytes(path: &Utf8Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, bytes)
}

/// Where a built mod lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    /// `Data/<name>` of the game install (Core and expansions).
    Data,
    /// The install `Mods` folder.
    Mods,
    /// The Steam Workshop content folder.
    Workshop,
    /// The custom mod folder.
    Custom,
}

/// A mod that was written to disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltMod {
    /// The folder name.
    pub folder_name: String,
    /// The package id.
    pub package_id: String,
    /// The mod root.
    pub path: Utf8PathBuf,
    /// Which source folder holds it.
    pub location: Location,
}

/// The folders of a written install.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltInstall {
    /// The directory everything was written into.
    pub root: Utf8PathBuf,
    /// The game install folder (`steamapps/common/RimWorld` below the root).
    pub game_dir: Utf8PathBuf,
    /// `Data` inside the game folder.
    pub data_dir: Utf8PathBuf,
    /// `Mods` inside the game folder.
    pub mods_dir: Utf8PathBuf,
    /// The Workshop content folder of the game (`steamapps/workshop/content/294100`).
    pub workshop_dir: Utf8PathBuf,
    /// The custom mod folder.
    pub custom_dir: Utf8PathBuf,
    /// The user data folder (holds `Config/ModsConfig.xml`).
    pub user_data_dir: Utf8PathBuf,
    /// Every mod written, in the order Core, expansions, Mods, Workshop, custom.
    pub mods: Vec<BuiltMod>,
}

impl BuiltInstall {
    /// The path of the mod in folder `folder_name`, wherever it lives.
    #[must_use]
    pub fn mod_path(&self, folder_name: &str) -> Option<&Utf8Path> {
        self.mods
            .iter()
            .find(|m| m.folder_name == folder_name)
            .map(|m| m.path.as_path())
    }

    /// The path of the `ModsConfig.xml` file (it exists only after `active_mods`).
    #[must_use]
    pub fn mods_config_path(&self) -> Utf8PathBuf {
        self.user_data_dir.join("Config/ModsConfig.xml")
    }

    /// The path of `Version.txt`.
    #[must_use]
    pub fn version_file(&self) -> Utf8PathBuf {
        self.game_dir.join("Version.txt")
    }
}

/// A [`BuiltInstall`] in a temporary directory that is removed on drop.
#[derive(Debug)]
pub struct TempInstall {
    _dir: tempfile::TempDir,
    install: BuiltInstall,
}

impl Deref for TempInstall {
    type Target = BuiltInstall;

    fn deref(&self) -> &BuiltInstall {
        &self.install
    }
}

/// Builds a fictional install on disk. See the module documentation.
#[derive(Debug, Clone)]
pub struct InstallBuilder {
    game_version: String,
    core: ModFolder,
    dlc: Vec<ModFolder>,
    mods: Vec<ModFolder>,
    workshop: Vec<(u64, ModFolder)>,
    custom: Vec<ModFolder>,
    active: Option<Vec<String>>,
}

impl Default for InstallBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl InstallBuilder {
    /// An install with a Core folder (package id `ludeon.rimworld`) and nothing else.
    #[must_use]
    pub fn new() -> Self {
        let core = ModFolder::new("Core", rimstudio_core::paths::CORE_PACKAGE_ID)
            .name("Core")
            .author("RS Studio")
            .description("Fictional core content for tests.");
        Self {
            game_version: DEFAULT_GAME_VERSION.to_owned(),
            core,
            dlc: Vec::new(),
            mods: Vec::new(),
            workshop: Vec::new(),
            custom: Vec::new(),
            active: None,
        }
    }

    /// Sets the text of `Version.txt` and the version Core declares (major.minor).
    #[must_use]
    pub fn game_version(mut self, version: impl Into<String>) -> Self {
        self.game_version = version.into();
        self
    }

    /// Adds a definition to Core's default definitions file.
    #[must_use]
    pub fn core_def(mut self, node: impl Into<Node>) -> Self {
        self.core = self.core.def(node);
        self
    }

    /// Adds a definitions file to Core (`Data/Core/Defs/<file>`).
    #[must_use]
    pub fn core_defs_file(mut self, file: &str, nodes: Vec<Node>) -> Self {
        self.core = self.core.defs_file(file, nodes);
        self
    }

    /// Adds a patch file to Core.
    #[must_use]
    pub fn core_patch_file(mut self, file: &str, operations: Vec<Node>) -> Self {
        self.core = self.core.patch_file(file, operations);
        self
    }

    /// Changes the Core folder description with a closure over its [`ModFolder`].
    #[must_use]
    pub fn core(mut self, edit: impl FnOnce(ModFolder) -> ModFolder) -> Self {
        self.core = edit(self.core);
        self
    }

    /// Adds an expansion folder to `Data` (the folder name is the `Data` sub folder).
    #[must_use]
    pub fn dlc(mut self, folder: ModFolder) -> Self {
        self.dlc.push(folder);
        self
    }

    /// Adds a mod to the install `Mods` folder.
    #[must_use]
    pub fn mod_folder(mut self, folder: ModFolder) -> Self {
        self.mods.push(folder);
        self
    }

    /// Adds a mod to the Workshop content folder, in a folder named after the Workshop id.
    #[must_use]
    pub fn workshop_mod(mut self, workshop_id: u64, mut folder: ModFolder) -> Self {
        folder.folder_name = workshop_id.to_string();
        self.workshop.push((workshop_id, folder));
        self
    }

    /// Adds a mod to the custom mod folder.
    #[must_use]
    pub fn custom_mod(mut self, folder: ModFolder) -> Self {
        self.custom.push(folder);
        self
    }

    /// Writes `Config/ModsConfig.xml` with these active package ids (Core is not added).
    #[must_use]
    pub fn active_mods(mut self, ids: &[&str]) -> Self {
        self.active = Some(ids.iter().map(|i| (*i).to_owned()).collect());
        self
    }

    /// Writes the install below `dir` (created when missing).
    ///
    /// # Errors
    /// Any file system error, or [`io::ErrorKind::InvalidInput`] when `dir` is not valid UTF-8.
    pub fn build(&self, dir: impl AsRef<Path>) -> io::Result<BuiltInstall> {
        let root = Utf8PathBuf::from_path_buf(dir.as_ref().to_path_buf()).map_err(|p| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("not a UTF-8 path: {}", p.display()),
            )
        })?;
        let game_dir = root.join("steamapps/common/RimWorld");
        let data_dir = game_dir.join("Data");
        let mods_dir = game_dir.join("Mods");
        let workshop_dir = root.join("steamapps/workshop/content/294100");
        let custom_dir = root.join("custom");
        let user_data_dir = root.join("userdata");
        for d in [
            &data_dir,
            &mods_dir,
            &workshop_dir,
            &custom_dir,
            &user_data_dir,
        ] {
            fs::create_dir_all(d)?;
        }
        write_bytes(&game_dir.join("Version.txt"), self.game_version.as_bytes())?;
        write_bytes(&game_dir.join("RimWorldLinux"), b"")?;

        let major_minor = self
            .game_version
            .split_whitespace()
            .next()
            .unwrap_or("")
            .split('.')
            .take(2)
            .collect::<Vec<_>>()
            .join(".");
        let mut core = self.core.clone();
        if core.spec.supported_versions == ["1.6"] && !major_minor.is_empty() {
            core.spec.supported_versions = vec![major_minor];
        }

        let mut built = Vec::new();
        let mut put =
            |folder: &ModFolder, parent: &Utf8Path, location: Location| -> io::Result<()> {
                let path = folder.write_to(parent)?;
                built.push(BuiltMod {
                    folder_name: folder.folder_name.clone(),
                    package_id: folder.package_id.clone(),
                    path,
                    location,
                });
                Ok(())
            };
        put(&core, &data_dir, Location::Data)?;
        for f in &self.dlc {
            put(f, &data_dir, Location::Data)?;
        }
        for f in &self.mods {
            put(f, &mods_dir, Location::Mods)?;
        }
        for (_, f) in &self.workshop {
            put(f, &workshop_dir, Location::Workshop)?;
        }
        for f in &self.custom {
            put(f, &custom_dir, Location::Custom)?;
        }

        if let Some(active) = &self.active {
            let data = ModsConfigData {
                version: Some(self.game_version.clone()),
                active_mods: active.clone(),
                known_expansions: Some(Vec::new()),
            };
            write_bytes(
                &user_data_dir.join("Config/ModsConfig.xml"),
                mods_config::create(&data, &RenderOpts::default()).as_bytes(),
            )?;
        }
        Ok(BuiltInstall {
            root,
            game_dir,
            data_dir,
            mods_dir,
            workshop_dir,
            custom_dir,
            user_data_dir,
            mods: built,
        })
    }

    /// Writes the install into a new temporary directory.
    ///
    /// # Errors
    /// Any file system error.
    pub fn build_temp(&self) -> io::Result<TempInstall> {
        let dir = tempfile::tempdir()?;
        let install = self.build(dir.path())?;
        Ok(TempInstall { _dir: dir, install })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_core::load_plan::LoadEntry;
    use rimstudio_core::tree::NodeBuilder;
    use rimstudio_xml::about::read_lenient;
    use rimstudio_xml::{ParseMode, parse_document};

    fn gun(name: &str) -> Node {
        NodeBuilder::new("ThingDef")
            .text_elem("defName", name)
            .build()
    }

    #[test]
    fn writes_the_install_layout() {
        let install = InstallBuilder::new()
            .core_def(gun("RS_CoreGun"))
            .dlc(ModFolder::new("Royalty", "ludeon.rimworld.royalty").def(gun("RS_RoyalGun")))
            .mod_folder(ModFolder::new("RS_TestMod", "rs.testmod").def(gun("RS_TestRifle")))
            .workshop_mod(1_000_000_001, ModFolder::new("ignored", "rs.shop"))
            .custom_mod(ModFolder::new("RS_Custom", "rs.custom"))
            .active_mods(&["ludeon.rimworld", "rs.testmod"])
            .build_temp()
            .unwrap();
        assert_eq!(
            fs::read_to_string(install.version_file()).unwrap(),
            DEFAULT_GAME_VERSION
        );
        for rel in [
            "Data/Core/About/About.xml",
            "Data/Core/Defs/RS_Defs.xml",
            "Data/Royalty/About/About.xml",
            "Mods/RS_TestMod/Defs/RS_Defs.xml",
        ] {
            assert!(install.game_dir.join(rel).is_file(), "{rel}");
        }
        assert!(
            install
                .workshop_dir
                .join("1000000001/About/About.xml")
                .is_file()
        );
        assert!(
            install
                .custom_dir
                .join("RS_Custom/About/About.xml")
                .is_file()
        );
        assert!(install.mods_config_path().is_file());
        assert_eq!(install.mods.len(), 5);
        assert_eq!(
            install.mod_path("RS_TestMod").unwrap(),
            install.mods_dir.join("RS_TestMod")
        );
        assert_eq!(install.mods[0].location, Location::Data);
    }

    #[test]
    fn core_declares_the_running_major_minor() {
        let install = InstallBuilder::new()
            .game_version("1.7.2000 rev1")
            .build_temp()
            .unwrap();
        let bytes = fs::read(install.data_dir.join("Core/About/About.xml")).unwrap();
        let read = read_lenient(&bytes);
        assert_eq!(read.about.package_id, "ludeon.rimworld");
        assert_eq!(read.about.supported_versions, ["1.7"]);
    }

    #[test]
    fn generated_files_parse_in_game_mode() {
        let install = InstallBuilder::new()
            .mod_folder(
                ModFolder::new("RS_TestMod", "rs.testmod")
                    .def(gun("RS_A"))
                    .def(gun("RS_B"))
                    .patch_file(
                        "P.xml",
                        vec![
                            NodeBuilder::new("Operation")
                                .attr("Class", "PatchOperationAdd")
                                .build(),
                        ],
                    )
                    .load_folders(LoadFoldersSpec::from_blocks([(
                        "1.6",
                        vec![LoadEntry::root()],
                    )])),
            )
            .build_temp()
            .unwrap();
        let root = install.mod_path("RS_TestMod").unwrap();
        for rel in [
            "Defs/RS_Defs.xml",
            "Patches/P.xml",
            "LoadFolders.xml",
            "About/About.xml",
        ] {
            let bytes = fs::read(root.join(rel)).unwrap();
            let doc = parse_document(&bytes, ParseMode::Game).unwrap();
            assert!(doc.diagnostics.is_empty(), "{rel}");
        }
        let defs = parse_document(
            &fs::read(root.join("Defs/RS_Defs.xml")).unwrap(),
            ParseMode::Game,
        )
        .unwrap();
        assert_eq!(defs.root.elements().count(), 2);
    }

    #[test]
    fn quirks_change_names_encoding_and_by_version_lists() {
        let rel = VersionRelations {
            load_after: vec!["rs.six".into()],
            mod_dependencies: vec![ModDependency {
                package_id: "rs.dep".into(),
                display_name: "Dep".into(),
                download_url: Some("https://example.invalid".into()),
                ..ModDependency::default()
            }],
            ..VersionRelations::default()
        };
        let install = InstallBuilder::new()
            .mod_folder(
                ModFolder::new("RS_Quirky", "rs.quirky")
                    .lowercase_about_dir()
                    .about_file_name("about.xml")
                    .load_folders_file_name("loadFolders.xml")
                    .load_folders(LoadFoldersSpec::from_blocks([(
                        "1.6",
                        vec![LoadEntry::root()],
                    )]))
                    .bom()
                    .crlf()
                    .by_version("1.6", rel),
            )
            .build_temp()
            .unwrap();
        let root = install.mod_path("RS_Quirky").unwrap();
        assert!(!root.join("About").exists());
        let bytes = fs::read(root.join("about/about.xml")).unwrap();
        assert!(bytes.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert!(String::from_utf8_lossy(&bytes).contains("\r\n"));
        let about = read_lenient(&bytes).about;
        assert_eq!(about.by_version["1.6"].load_after, ["rs.six"]);
        assert_eq!(
            about.by_version["1.6"].mod_dependencies[0].package_id,
            "rs.dep"
        );
        assert!(root.join("loadFolders.xml").is_file());
    }

    #[test]
    fn raw_files_and_missing_about_support_broken_mod_fixtures() {
        let install = InstallBuilder::new()
            .mod_folder(
                ModFolder::new("RS_Broken", "rs.broken")
                    .raw_about("<ModMetaData><name>".as_bytes().to_vec())
                    .raw_file("Defs/Bad.xml", b"<Defs><a></Defs>".to_vec())
                    .empty_dir("Textures"),
            )
            .mod_folder(ModFolder::new("RS_NoAbout", "rs.noabout").no_about())
            .build_temp()
            .unwrap();
        let broken = install.mod_path("RS_Broken").unwrap();
        assert!(broken.join("Textures").is_dir());
        assert!(
            parse_document(
                &fs::read(broken.join("Defs/Bad.xml")).unwrap(),
                ParseMode::Game
            )
            .is_err()
        );
        assert!(
            !install
                .mod_path("RS_NoAbout")
                .unwrap()
                .join("About")
                .exists()
        );
    }

    #[test]
    fn building_twice_gives_identical_files() {
        let builder = InstallBuilder::new()
            .core_def(gun("RS_CoreGun"))
            .mod_folder(
                ModFolder::new("RS_TestMod", "rs.testmod")
                    .depends_on("rs.base", "Base")
                    .def(gun("RS_X")),
            );
        let a = builder.build_temp().unwrap();
        let b = builder.build_temp().unwrap();
        for rel in [
            "Mods/RS_TestMod/About/About.xml",
            "Mods/RS_TestMod/Defs/RS_Defs.xml",
            "Data/Core/Defs/RS_Defs.xml",
        ] {
            assert_eq!(
                fs::read(a.game_dir.join(rel)).unwrap(),
                fs::read(b.game_dir.join(rel)).unwrap()
            );
        }
    }
}
