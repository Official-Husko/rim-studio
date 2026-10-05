//! `About/About.xml`: lenient reading, targeted span edits and creation of new files.
//!
//! The reader follows the game's `ModMetaData` loading: field names are matched case sensitively,
//! a text field that holds a comment or child elements is dropped, `ByVersion` blocks are keyed by
//! the lower cased version with a leading `v` removed and the first occurrence wins, and a file
//! that cannot be parsed yields the default record. Problems are reported as warnings, never as
//! failures. The edit helpers change one field by splicing bytes and leave the rest of the file
//! exactly as it was.

use std::borrow::Cow;
use std::collections::BTreeMap;

use camino::Utf8PathBuf;
use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};
use rimstudio_core::error::CoreError;
use rimstudio_core::ids::{PackageId, SourceId};
use rimstudio_core::mods::{ModDependency, ModMeta, VersionRelations};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_core::version::normalize_version_key;

use crate::edit::SpanEditor;
use crate::error::{XmlError, XmlResult, codes};
use crate::modes::ParseMode;
use crate::render::{RenderOpts, render};
use crate::walk::{Sink, walk};

/// Everything `About.xml` says, as the game would read it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct About {
    /// `packageId`, trimmed, as written (empty when absent or dropped).
    pub package_id: String,
    /// `name`, trimmed.
    pub name: String,
    /// `shortName`.
    pub short_name: Option<String>,
    /// `author` (one text, split for display by [`About::author_list`]).
    pub author: Option<String>,
    /// `authors` entries.
    pub authors: Vec<String>,
    /// `description` as written (not trimmed).
    pub description: String,
    /// `descriptionsByVersion`, keyed by the normalised version key.
    pub descriptions_by_version: BTreeMap<String, String>,
    /// `supportedVersions` entries as written, trimmed.
    pub supported_versions: Vec<String>,
    /// True when the file has a `supportedVersions` element at all.
    pub has_supported_versions: bool,
    /// `url`.
    pub url: Option<String>,
    /// `modVersion`.
    pub mod_version: Option<String>,
    /// `modIconPath`.
    pub mod_icon_path: Option<String>,
    /// `steamAppId`.
    pub steam_app_id: Option<u32>,
    /// `loadAfter` entries.
    pub load_after: Vec<String>,
    /// `loadBefore` entries.
    pub load_before: Vec<String>,
    /// `forceLoadAfter` entries.
    pub force_load_after: Vec<String>,
    /// `forceLoadBefore` entries.
    pub force_load_before: Vec<String>,
    /// `incompatibleWith` entries.
    pub incompatible_with: Vec<String>,
    /// `modDependencies` entries.
    pub mod_dependencies: Vec<ModDependency>,
    /// The per version blocks, keyed by the normalised version key.
    pub by_version: BTreeMap<String, VersionRelations>,
    /// Names of root children that are not fields the game reads, in file order.
    pub unknown_tags: Vec<String>,
}

impl About {
    /// The authors as the game shows them: the `authors` list when it has entries, else `author`
    /// split on commas and the word `and`.
    #[must_use]
    pub fn author_list(&self) -> Vec<String> {
        if !self.authors.is_empty() {
            return self.authors.clone();
        }
        let Some(author) = &self.author else {
            return Vec::new();
        };
        author
            .replace(" and ", ",")
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    }

    /// Builds the core mod record. An empty `name` falls back to the folder name of `path`.
    ///
    /// # Errors
    /// [`CoreError`] when the package id is empty or not usable as an id.
    pub fn into_meta(self, source: SourceId, path: Utf8PathBuf) -> Result<ModMeta, CoreError> {
        let package_id = PackageId::parse(&self.package_id)?;
        let name = if self.name.trim().is_empty() {
            path.file_name().unwrap_or("").to_owned()
        } else {
            self.name.clone()
        };
        let authors = self.author_list();
        let mut meta = ModMeta::new(package_id, name, source, path);
        meta.authors = authors;
        meta.description = self.description;
        meta.url = self.url.filter(|u| !u.is_empty());
        meta.supported_versions = self.supported_versions;
        meta.load_after = self.load_after;
        meta.load_before = self.load_before;
        meta.force_load_after = self.force_load_after;
        meta.force_load_before = self.force_load_before;
        meta.incompatible_with = self.incompatible_with;
        meta.mod_dependencies = self.mod_dependencies;
        meta.by_version = self.by_version;
        Ok(meta)
    }
}

/// The result of [`read_lenient`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AboutRead {
    /// What the file says (the default record when the file could not be parsed).
    pub about: About,
    /// False when the file was not parseable at all and `about` is the default record.
    pub parsed: bool,
    /// Warnings found while reading.
    pub warnings: Vec<Diagnostic>,
}

/// Switches of the About reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AboutOptions {
    /// Also accept field names that differ from the game's only in letter case. The game ignores
    /// such elements, so this is off by default; it is useful for showing what an author meant.
    pub case_insensitive_fields: bool,
}

/// One element with its text, children and whether a comment sat directly inside it.
#[derive(Debug, Default)]
struct Raw {
    name: String,
    text: String,
    kids: Vec<Raw>,
    has_comment: bool,
}

#[derive(Default)]
struct RawSink {
    stack: Vec<Raw>,
    root: Option<Raw>,
}

impl Sink for RawSink {
    fn start(&mut self, tag: &str, _attrs: Vec<(String, String)>) {
        self.stack.push(Raw {
            name: tag.to_owned(),
            ..Raw::default()
        });
    }

    fn end(&mut self) {
        if let Some(done) = self.stack.pop() {
            match self.stack.last_mut() {
                Some(parent) => parent.kids.push(done),
                None => self.root = Some(done),
            }
        }
    }

    fn text(&mut self, text: String) {
        if let Some(top) = self.stack.last_mut() {
            top.text.push_str(&text);
        }
    }

    fn comment(&mut self) {
        if let Some(top) = self.stack.last_mut() {
            top.has_comment = true;
        }
    }
}

/// Converts UTF-16 and UTF-32 input with a byte order mark to UTF-8 (the game reads About.xml with
/// `File.ReadAllText`, which honours those marks); other input is returned unchanged.
pub(crate) fn normalise_encoding(bytes: &[u8]) -> Cow<'_, [u8]> {
    let utf16 = |data: &[u8], le: bool| -> Vec<u8> {
        let units: Vec<u16> = data
            .chunks_exact(2)
            .map(|c| {
                if le {
                    u16::from_le_bytes([c[0], c[1]])
                } else {
                    u16::from_be_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units).into_bytes()
    };
    let utf32 = |data: &[u8], le: bool| -> Vec<u8> {
        data.chunks_exact(4)
            .map(|c| {
                let v = if le {
                    u32::from_le_bytes([c[0], c[1], c[2], c[3]])
                } else {
                    u32::from_be_bytes([c[0], c[1], c[2], c[3]])
                };
                char::from_u32(v).unwrap_or('\u{FFFD}')
            })
            .collect::<String>()
            .into_bytes()
    };
    match bytes {
        [0xFF, 0xFE, 0x00, 0x00, rest @ ..] => Cow::Owned(utf32(rest, true)),
        [0x00, 0x00, 0xFE, 0xFF, rest @ ..] => Cow::Owned(utf32(rest, false)),
        [0xFF, 0xFE, rest @ ..] => Cow::Owned(utf16(rest, true)),
        [0xFE, 0xFF, rest @ ..] => Cow::Owned(utf16(rest, false)),
        _ => Cow::Borrowed(bytes),
    }
}

/// Reads an `About.xml` leniently with the default options (see [`AboutOptions`]).
///
/// Never fails: a file that cannot be parsed gives the default record, `parsed == false` and an
/// error diagnostic, as the game falls back to defaults.
#[must_use]
pub fn read_lenient(bytes: &[u8]) -> AboutRead {
    read_lenient_with(bytes, &AboutOptions::default())
}

/// [`read_lenient`] with explicit options.
#[must_use]
pub fn read_lenient_with(bytes: &[u8], options: &AboutOptions) -> AboutRead {
    let data = normalise_encoding(bytes);
    let mut sink = RawSink::default();
    let walked = walk(&data, ParseMode::Tolerant, &mut sink);
    let mut warnings = Vec::new();
    let info = match walked {
        Ok(info) => info,
        Err(err) => {
            warnings.push(err.to_diagnostic());
            return AboutRead {
                about: About::default(),
                parsed: false,
                warnings,
            };
        }
    };
    warnings.extend(info.diagnostics);
    let Some(root) = sink.root else {
        warnings.push(XmlError::NoRoot.to_diagnostic());
        return AboutRead {
            about: About::default(),
            parsed: false,
            warnings,
        };
    };
    let mut reader = FieldReader {
        about: About::default(),
        warnings,
        seen: Vec::new(),
        ignore_case: options.case_insensitive_fields,
    };
    reader.read_root(&root);
    if !reader.about.has_supported_versions {
        reader.warn(
            codes::ABOUT_NO_VERSIONS,
            Severity::Warning,
            "the file has no supportedVersions list".to_owned(),
        );
    }
    AboutRead {
        about: reader.about,
        parsed: true,
        warnings: reader.warnings,
    }
}

const FIELDS: &[&str] = &[
    "name",
    "shortName",
    "author",
    "authors",
    "packageId",
    "description",
    "descriptionsByVersion",
    "supportedVersions",
    "targetVersion",
    "modVersion",
    "url",
    "modIconPath",
    "steamAppId",
    "modDependencies",
    "loadBefore",
    "loadAfter",
    "forceLoadBefore",
    "forceLoadAfter",
    "incompatibleWith",
    "modDependenciesByVersion",
    "loadBeforeByVersion",
    "loadAfterByVersion",
    "forceLoadBeforeByVersion",
    "forceLoadAfterByVersion",
    "incompatibleWithByVersion",
    "modLatestUpdateDate",
];

struct FieldReader {
    about: About,
    warnings: Vec<Diagnostic>,
    seen: Vec<&'static str>,
    ignore_case: bool,
}

impl FieldReader {
    fn warn(&mut self, code: DiagCode, severity: Severity, message: String) {
        self.warnings.push(Diagnostic::new(code, severity, message));
    }

    fn read_root(&mut self, root: &Raw) {
        for kid in &root.kids {
            let exact = FIELDS.iter().copied().find(|f| *f == kid.name);
            let field = match exact {
                Some(f) => Some(f),
                None => {
                    let folded = FIELDS
                        .iter()
                        .copied()
                        .find(|f| f.eq_ignore_ascii_case(&kid.name));
                    if let Some(f) = folded {
                        self.warn(
                            codes::ABOUT_TAG_CASE,
                            Severity::Warning,
                            format!(
                                "element <{}> differs from the field name <{f}> only in letter case; the game ignores it",
                                kid.name
                            ),
                        );
                        if self.ignore_case { Some(f) } else { None }
                    } else {
                        None
                    }
                }
            };
            let Some(field) = field else {
                if !FIELDS.iter().any(|f| f.eq_ignore_ascii_case(&kid.name)) {
                    self.warn(
                        codes::ABOUT_UNKNOWN_TAG,
                        Severity::Info,
                        format!("element <{}> is not a field the game reads", kid.name),
                    );
                }
                self.about.unknown_tags.push(kid.name.clone());
                continue;
            };
            if self.seen.contains(&field) {
                self.warn(
                    codes::ABOUT_DUPLICATE,
                    Severity::Warning,
                    format!("field <{field}> appears more than once; the first one is used"),
                );
                continue;
            }
            self.seen.push(field);
            self.read_field(field, kid);
        }
    }

    /// The text of a string field, or `None` when the game would drop it.
    fn string_value(&mut self, field: &str, raw: &Raw) -> Option<String> {
        if raw.has_comment || !raw.kids.is_empty() {
            self.warn(
                codes::ABOUT_FIELD_DROPPED,
                Severity::Warning,
                format!("field <{field}> holds a comment or child elements, so the game ignores its value"),
            );
            None
        } else {
            Some(raw.text.clone())
        }
    }

    fn trimmed(&mut self, field: &str, raw: &Raw) -> Option<String> {
        self.string_value(field, raw).map(|s| s.trim().to_owned())
    }

    fn list(&mut self, field: &str, raw: &Raw) -> Vec<String> {
        let mut out = Vec::new();
        for kid in &raw.kids {
            if kid.name != "li" {
                self.warn(
                    codes::ABOUT_UNKNOWN_TAG,
                    Severity::Info,
                    format!("list <{field}> holds <{}> instead of <li>", kid.name),
                );
                continue;
            }
            if let Some(v) = self.trimmed(field, kid)
                && !v.is_empty()
            {
                out.push(v);
            }
        }
        out
    }

    fn dependency(&mut self, raw: &Raw) -> Option<ModDependency> {
        let mut dep = ModDependency::default();
        for kid in &raw.kids {
            match kid.name.as_str() {
                "packageId" => dep.package_id = self.trimmed("packageId", kid).unwrap_or_default(),
                "displayName" => {
                    dep.display_name = self.trimmed("displayName", kid).unwrap_or_default();
                }
                "steamWorkshopUrl" => {
                    dep.steam_workshop_url = self.trimmed("steamWorkshopUrl", kid);
                }
                "downloadUrl" => dep.download_url = self.trimmed("downloadUrl", kid),
                "alternativePackageIds" => {
                    dep.alternatives = self.list("alternativePackageIds", kid);
                }
                _ => {}
            }
        }
        if dep.package_id.is_empty() {
            self.warn(
                codes::ABOUT_DEPENDENCY_NO_ID,
                Severity::Warning,
                "a dependency has no packageId and is skipped".to_owned(),
            );
            return None;
        }
        Some(dep)
    }

    fn dependencies(&mut self, raw: &Raw) -> Vec<ModDependency> {
        let mut out = Vec::new();
        for kid in &raw.kids {
            if kid.name != "li" {
                continue;
            }
            if let Some(d) = self.dependency(kid) {
                out.push(d);
            }
        }
        out
    }

    fn read_field(&mut self, field: &'static str, raw: &Raw) {
        match field {
            "name" => self.about.name = self.trimmed(field, raw).unwrap_or_default(),
            "shortName" => self.about.short_name = self.trimmed(field, raw),
            "author" => self.about.author = self.trimmed(field, raw),
            "authors" => self.about.authors = self.list(field, raw),
            "packageId" => self.about.package_id = self.trimmed(field, raw).unwrap_or_default(),
            "description" => {
                self.about.description = self.string_value(field, raw).unwrap_or_default()
            }
            "supportedVersions" => {
                self.about.has_supported_versions = true;
                self.about.supported_versions = self.list(field, raw);
            }
            "modVersion" => self.about.mod_version = self.trimmed(field, raw),
            "url" => self.about.url = self.trimmed(field, raw),
            "modIconPath" => self.about.mod_icon_path = self.trimmed(field, raw),
            "steamAppId" => {
                self.about.steam_app_id =
                    self.trimmed(field, raw).and_then(|s| s.parse::<u32>().ok());
            }
            "loadAfter" => self.about.load_after = self.list(field, raw),
            "loadBefore" => self.about.load_before = self.list(field, raw),
            "forceLoadAfter" => self.about.force_load_after = self.list(field, raw),
            "forceLoadBefore" => self.about.force_load_before = self.list(field, raw),
            "incompatibleWith" => self.about.incompatible_with = self.list(field, raw),
            "modDependencies" => self.about.mod_dependencies = self.dependencies(raw),
            "descriptionsByVersion" => self.read_descriptions(raw),
            "modDependenciesByVersion" => self.read_by_version(field, raw),
            "loadBeforeByVersion" => self.read_by_version(field, raw),
            "loadAfterByVersion" => self.read_by_version(field, raw),
            "forceLoadBeforeByVersion" => self.read_by_version(field, raw),
            "forceLoadAfterByVersion" => self.read_by_version(field, raw),
            "incompatibleWithByVersion" => self.read_by_version(field, raw),
            _ => {}
        }
    }

    fn read_descriptions(&mut self, raw: &Raw) {
        for kid in &raw.kids {
            let key = normalize_version_key(&kid.name);
            if self.about.descriptions_by_version.contains_key(&key) {
                self.warn(
                    codes::ABOUT_DUPLICATE_VERSION,
                    Severity::Warning,
                    format!("descriptionsByVersion repeats the key {key}; the first one is used"),
                );
                continue;
            }
            if let Some(text) = self.string_value("descriptionsByVersion", kid) {
                self.about.descriptions_by_version.insert(key, text);
            }
        }
    }

    fn read_by_version(&mut self, field: &'static str, raw: &Raw) {
        let mut seen_keys: Vec<String> = Vec::new();
        for kid in &raw.kids {
            let key = normalize_version_key(&kid.name);
            if seen_keys.contains(&key) {
                self.warn(
                    codes::ABOUT_DUPLICATE_VERSION,
                    Severity::Warning,
                    format!("{field} repeats the key {key}; the first one is used"),
                );
                continue;
            }
            seen_keys.push(key.clone());
            let (list, deps) = if field == "modDependenciesByVersion" {
                (Vec::new(), self.dependencies(kid))
            } else {
                (self.list(field, kid), Vec::new())
            };
            let entry = self.about.by_version.entry(key).or_default();
            match field {
                "modDependenciesByVersion" => entry.mod_dependencies = deps,
                "loadBeforeByVersion" => entry.load_before = list,
                "loadAfterByVersion" => entry.load_after = list,
                "forceLoadBeforeByVersion" => entry.force_load_before = list,
                "forceLoadAfterByVersion" => entry.force_load_after = list,
                "incompatibleWithByVersion" => entry.incompatible_with = list,
                _ => {}
            }
        }
    }
}

/// True when white space comes before the XML declaration (after an optional byte order mark): the game's
/// parser rejects such a file.
#[must_use]
pub fn has_whitespace_before_declaration(text: &str) -> bool {
    let body = text.trim_start_matches('\u{feff}');
    body.starts_with(char::is_whitespace) && body.trim_start().starts_with("<?xml")
}

// ---------------------------------------------------------------------------------------- edits

fn root_path(ed: &SpanEditor) -> String {
    format!("/{}", ed.root_tag())
}

fn li_nodes(items: &[String]) -> Vec<Node> {
    items
        .iter()
        .map(|i| Node::with_text("li", i.clone()))
        .collect()
}

/// Sets a text field (`name`, `packageId`, `description` ...) of an About text: the element is
/// updated in place, or appended to the root when it does not exist.
///
/// # Errors
/// [`XmlError`] when `text` is not a well formed document or `field` is not an XML name.
pub fn set_text_field(text: &str, field: &str, value: &str) -> XmlResult<String> {
    let mut ed = SpanEditor::open(text)?;
    let root = root_path(&ed);
    let path = format!("{root}/{field}");
    if ed.exists(&path) {
        if ed.element(&path)?.child_count > 0 {
            ed.replace_element(&path, &Node::with_text(field, value))?;
        } else {
            ed.replace_text(&path, value)?;
        }
    } else {
        ed.insert_child(&root, &Node::with_text(field, value))?;
    }
    Ok(ed.into_text())
}

/// Sets a list field (`supportedVersions`, `loadAfter` ...) to `items`, one `li` each. The element
/// is created at the end of the root when it does not exist.
///
/// # Errors
/// [`XmlError`] when `text` is not a well formed document or `field` is not an XML name.
pub fn set_list_field(text: &str, field: &str, items: &[String]) -> XmlResult<String> {
    let mut ed = SpanEditor::open(text)?;
    let root = root_path(&ed);
    let path = format!("{root}/{field}");
    if ed.exists(&path) {
        ed.replace_children(&path, &li_nodes(items))?;
    } else {
        let node = Node {
            tag: field.to_owned(),
            attrs: Vec::new(),
            children: li_nodes(items)
                .into_iter()
                .map(rimstudio_core::tree::Child::Element)
                .collect(),
        };
        ed.insert_child(&root, &node)?;
    }
    Ok(ed.into_text())
}

/// Sets `packageId`.
///
/// # Errors
/// As [`set_text_field`].
pub fn set_package_id(text: &str, id: &str) -> XmlResult<String> {
    set_text_field(text, "packageId", id)
}

/// Sets `name`.
///
/// # Errors
/// As [`set_text_field`].
pub fn set_name(text: &str, name: &str) -> XmlResult<String> {
    set_text_field(text, "name", name)
}

/// Sets `description`.
///
/// # Errors
/// As [`set_text_field`].
pub fn set_description(text: &str, description: &str) -> XmlResult<String> {
    set_text_field(text, "description", description)
}

/// Sets `supportedVersions`.
///
/// # Errors
/// As [`set_list_field`].
pub fn set_supported_versions(text: &str, versions: &[String]) -> XmlResult<String> {
    set_list_field(text, "supportedVersions", versions)
}

/// Sets `loadAfter`.
///
/// # Errors
/// As [`set_list_field`].
pub fn set_load_after(text: &str, ids: &[String]) -> XmlResult<String> {
    set_list_field(text, "loadAfter", ids)
}

/// Adds one `li` to a list field unless an entry with the same text (ignoring case and surrounding
/// white space) is already there. The list is created when missing.
///
/// # Errors
/// As [`set_list_field`].
pub fn add_list_entry(text: &str, field: &str, entry: &str) -> XmlResult<String> {
    let mut ed = SpanEditor::open(text)?;
    let root = root_path(&ed);
    let path = format!("{root}/{field}");
    if !ed.exists(&path) {
        return set_list_field(text, field, &[entry.to_owned()]);
    }
    for child in ed.children(&path)? {
        if child.tag == "li"
            && ed
                .element_text(&child.path)?
                .trim()
                .eq_ignore_ascii_case(entry.trim())
        {
            return Ok(text.to_owned());
        }
    }
    ed.insert_child(&path, &Node::with_text("li", entry))?;
    Ok(ed.into_text())
}

/// Adds a `loadAfter` entry (see [`add_list_entry`]).
///
/// # Errors
/// As [`add_list_entry`].
pub fn add_load_after(text: &str, id: &str) -> XmlResult<String> {
    add_list_entry(text, "loadAfter", id)
}

/// Adds a `supportedVersions` entry (see [`add_list_entry`]).
///
/// # Errors
/// As [`add_list_entry`].
pub fn add_supported_version(text: &str, version: &str) -> XmlResult<String> {
    add_list_entry(text, "supportedVersions", version)
}

// ---------------------------------------------------------------------------------------- create

/// What a new `About.xml` should say.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AboutSpec {
    /// `name`.
    pub name: String,
    /// `author`.
    pub author: String,
    /// `packageId`.
    pub package_id: String,
    /// `description`.
    pub description: String,
    /// `supportedVersions`.
    pub supported_versions: Vec<String>,
    /// `url`, written when present.
    pub url: Option<String>,
    /// `modVersion`, written when present.
    pub mod_version: Option<String>,
    /// `modDependencies`.
    pub mod_dependencies: Vec<ModDependency>,
    /// `loadAfter`.
    pub load_after: Vec<String>,
    /// `loadBefore`.
    pub load_before: Vec<String>,
    /// `incompatibleWith`.
    pub incompatible_with: Vec<String>,
}

fn dependency_node(dep: &ModDependency) -> Node {
    NodeBuilder::new("li")
        .text_elem("packageId", dep.package_id.clone())
        .text_elem("displayName", dep.display_name.clone())
        .text_elem_opt("steamWorkshopUrl", dep.steam_workshop_url.clone())
        .text_elem_opt("downloadUrl", dep.download_url.clone())
        .when(!dep.alternatives.is_empty(), |b| {
            b.elem("alternativePackageIds", |a| {
                a.li_each(dep.alternatives.clone())
            })
        })
        .build()
}

/// Builds the node tree of a new `About.xml` (root `ModMetaData`). Empty optional lists are not
/// written.
#[must_use]
pub fn create_about(spec: &AboutSpec) -> Node {
    NodeBuilder::new("ModMetaData")
        .text_elem("name", spec.name.clone())
        .text_elem("author", spec.author.clone())
        .text_elem("packageId", spec.package_id.clone())
        .text_elem_opt("url", spec.url.clone())
        .text_elem_opt("modVersion", spec.mod_version.clone())
        .elem("supportedVersions", |b| {
            b.li_each(spec.supported_versions.clone())
        })
        .when(!spec.mod_dependencies.is_empty(), |b| {
            b.elem("modDependencies", |d| {
                d.children(spec.mod_dependencies.iter().map(dependency_node))
            })
        })
        .when(!spec.load_after.is_empty(), |b| {
            b.elem("loadAfter", |l| l.li_each(spec.load_after.clone()))
        })
        .when(!spec.load_before.is_empty(), |b| {
            b.elem("loadBefore", |l| l.li_each(spec.load_before.clone()))
        })
        .when(!spec.incompatible_with.is_empty(), |b| {
            b.elem("incompatibleWith", |l| {
                l.li_each(spec.incompatible_with.clone())
            })
        })
        .text_elem("description", spec.description.clone())
        .build()
}

/// Renders a new `About.xml` file from a spec.
#[must_use]
pub fn render_about(spec: &AboutSpec, opts: &RenderOpts) -> String {
    render(&create_about(spec), opts)
}
