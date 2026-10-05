//! Stage S2: lenient About.xml parsing with three implementations (quick-xml events, quick-xml serde,
//! roxmltree DOM) that all produce the same typed struct.

use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, bincode::Encode, bincode::Decode, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct Dependency {
    pub package_id: String,
    pub display_name: String,
    pub steam_workshop_url: String,
    pub download_url: String,
    pub alternative_package_ids: Vec<String>,
}

/// Mirrors the fields of the game's ModMetaDataInternal (decompiled:Verse/ModMetaData.cs).
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize, bincode::Encode, bincode::Decode, rkyv::Archive, rkyv::Serialize, rkyv::Deserialize)]
pub struct About {
    pub package_id: String,
    pub name: String,
    pub short_name: String,
    pub author: String,
    pub authors: Vec<String>,
    pub description: String,
    pub url: String,
    pub mod_icon_path: String,
    pub mod_version: String,
    pub steam_app_id: String,
    pub target_version: String,
    pub supported_versions: Vec<String>,
    pub mod_dependencies: Vec<Dependency>,
    pub load_before: Vec<String>,
    pub load_after: Vec<String>,
    pub force_load_before: Vec<String>,
    pub force_load_after: Vec<String>,
    pub incompatible_with: Vec<String>,
    pub mod_dependencies_by_version: Vec<(String, Vec<Dependency>)>,
    pub load_before_by_version: Vec<(String, Vec<String>)>,
    pub load_after_by_version: Vec<(String, Vec<String>)>,
    pub incompatible_with_by_version: Vec<(String, Vec<String>)>,
    pub descriptions_by_version: Vec<(String, String)>,
}

// ---------------------------------------------------------------------------------------------
// Locating and decoding
// ---------------------------------------------------------------------------------------------

/// The game looks for `About/About.xml` and, failing that, compares file names inside the exact
/// `About` directory case-insensitively (decompiled:Verse/GenFile.cs ResolveCaseInsensitiveFilePath).
pub fn find_about(mod_root: &Path) -> Option<PathBuf> {
    let dir = mod_root.join("About");
    let direct = dir.join("About.xml");
    if direct.is_file() {
        return Some(direct);
    }
    for e in std::fs::read_dir(&dir).ok()?.flatten() {
        if e.file_name().to_string_lossy().eq_ignore_ascii_case("about.xml") {
            return Some(e.path());
        }
    }
    None
}

/// Strip a BOM, transcode UTF-16, and replace invalid UTF-8 instead of failing.
pub fn decode(bytes: &[u8]) -> Cow<'_, str> {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return String::from_utf8_lossy(rest);
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        let u: Vec<u16> = rest.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return Cow::Owned(String::from_utf16_lossy(&u));
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        let u: Vec<u16> = rest.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
        return Cow::Owned(String::from_utf16_lossy(&u));
    }
    String::from_utf8_lossy(bytes)
}

// ---------------------------------------------------------------------------------------------
// Field table (case-insensitive tag matching)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum F {
    PackageId,
    Name,
    ShortName,
    Author,
    Authors,
    Description,
    Url,
    ModIconPath,
    ModVersion,
    SteamAppId,
    TargetVersion,
    SupportedVersions,
    ModDependencies,
    LoadBefore,
    LoadAfter,
    ForceLoadBefore,
    ForceLoadAfter,
    IncompatibleWith,
    ModDependenciesByVersion,
    LoadBeforeByVersion,
    LoadAfterByVersion,
    IncompatibleWithByVersion,
    DescriptionsByVersion,
    Unknown,
}

const FIELD_NAMES: &[(&str, F)] = &[
    ("packageid", F::PackageId),
    ("name", F::Name),
    ("shortname", F::ShortName),
    ("author", F::Author),
    ("authors", F::Authors),
    ("description", F::Description),
    ("url", F::Url),
    ("modiconpath", F::ModIconPath),
    ("modversion", F::ModVersion),
    ("steamappid", F::SteamAppId),
    ("targetversion", F::TargetVersion),
    ("supportedversions", F::SupportedVersions),
    ("moddependencies", F::ModDependencies),
    ("loadbefore", F::LoadBefore),
    ("loadafter", F::LoadAfter),
    ("forceloadbefore", F::ForceLoadBefore),
    ("forceloadafter", F::ForceLoadAfter),
    ("incompatiblewith", F::IncompatibleWith),
    ("moddependenciesbyversion", F::ModDependenciesByVersion),
    ("loadbeforebyversion", F::LoadBeforeByVersion),
    ("loadafterbyversion", F::LoadAfterByVersion),
    ("incompatiblewithbyversion", F::IncompatibleWithByVersion),
    ("descriptionsbyversion", F::DescriptionsByVersion),
];

fn field_of(name: &[u8]) -> F {
    if name.len() > 28 {
        return F::Unknown;
    }
    for (n, f) in FIELD_NAMES {
        if n.len() == name.len() && n.as_bytes().eq_ignore_ascii_case(name) {
            return *f;
        }
    }
    F::Unknown
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum DF {
    PackageId,
    DisplayName,
    SteamWorkshopUrl,
    DownloadUrl,
    AltIds,
    Unknown,
}

fn dep_field_of(name: &[u8]) -> DF {
    let n = name.len();
    let eq = |s: &str| s.len() == n && s.as_bytes().eq_ignore_ascii_case(name);
    if eq("packageid") {
        DF::PackageId
    } else if eq("displayname") {
        DF::DisplayName
    } else if eq("steamworkshopurl") {
        DF::SteamWorkshopUrl
    } else if eq("downloadurl") {
        DF::DownloadUrl
    } else if eq("alternativepackageids") {
        DF::AltIds
    } else {
        DF::Unknown
    }
}

fn is_li(name: &[u8]) -> bool {
    name.len() == 2 && name.eq_ignore_ascii_case(b"li")
}

// ---------------------------------------------------------------------------------------------
// Variant A: quick-xml events (hand written state machine)
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq)]
enum Node {
    Root,
    Top(F),
    Li,
    Ver(usize),
    Dep(DF),
    Other,
}

#[derive(Debug, Default, Clone)]
pub struct ParseReport {
    /// parser reported an error; `About` holds what was read before it
    pub error: Option<String>,
    /// text contained an entity reference that XML does not define (for example &nbsp;)
    pub unknown_entities: u32,
}

pub fn parse_events(text: &str, strict: bool) -> (About, ParseReport) {
    let mut about = About::default();
    let mut rep = ParseReport::default();
    let mut rd = Reader::from_str(text);
    {
        let c = rd.config_mut();
        c.expand_empty_elements = true;
        if !strict {
            c.check_end_names = false;
            c.allow_unmatched_ends = true;
            c.allow_dangling_amp = true;
        }
    }
    let mut stack: Vec<Node> = Vec::with_capacity(8);
    // start offset into `buf` of the text collected for every open element
    let mut starts: Vec<usize> = Vec::with_capacity(8);
    let mut buf = String::new();
    loop {
        let ev = match rd.read_event() {
            Ok(e) => e,
            Err(e) => {
                rep.error = Some(e.to_string());
                break;
            }
        };
        match ev {
            Event::Start(e) => {
                let node = classify(&stack, &e, &mut about);
                stack.push(node);
                starts.push(buf.len());
            }
            Event::End(_) => {
                if let Some(node) = stack.pop() {
                    let s = starts.pop().unwrap_or(0).min(buf.len());
                    finish(&mut about, &stack, node, &buf[s..]);
                    buf.truncate(s);
                }
            }
            Event::Text(t) => {
                if matches!(stack.last(), Some(Node::Top(_) | Node::Li | Node::Ver(_) | Node::Dep(_))) {
                    buf.push_str(&t.xml10_content());
                }
            }
            Event::CData(c) => {
                if matches!(stack.last(), Some(Node::Top(_) | Node::Li | Node::Ver(_) | Node::Dep(_))) {
                    buf.push_str(&c.xml10_content());
                }
            }
            Event::GeneralRef(r) => {
                if !matches!(stack.last(), Some(Node::Top(_) | Node::Li | Node::Ver(_) | Node::Dep(_))) {
                    continue;
                }
                if r.is_char_ref() {
                    if let Ok(Some(ch)) = r.resolve_char_ref() {
                        buf.push(ch);
                    }
                } else {
                    match &*r {
                        "amp" => buf.push('&'),
                        "lt" => buf.push('<'),
                        "gt" => buf.push('>'),
                        "quot" => buf.push('"'),
                        "apos" => buf.push('\''),
                        other => {
                            rep.unknown_entities += 1;
                            buf.push('&');
                            buf.push_str(other);
                            buf.push(';');
                        }
                    }
                }
            }
            Event::Eof => {
                if !stack.is_empty() && rep.error.is_none() {
                    rep.error = Some("unexpected end of file".into());
                }
                break;
            }
            _ => {}
        }
    }
    // Elements still open at an error or at EOF: flush what was collected so far.
    while let Some(node) = stack.pop() {
        let s = starts.pop().unwrap_or(0).min(buf.len());
        finish(&mut about, &stack, node, &buf[s..]);
        buf.truncate(s);
    }
    (about, rep)
}

fn classify(stack: &[Node], e: &BytesStart, about: &mut About) -> Node {
    let name = e.name().into_inner().as_bytes();
    match stack {
        [] => Node::Root,
        [Node::Root] => match field_of(name) {
            F::Unknown => Node::Other,
            f => Node::Top(f),
        },
        [Node::Root, Node::Top(f)] => match f {
            F::SupportedVersions | F::Authors | F::LoadBefore | F::LoadAfter | F::ForceLoadBefore | F::ForceLoadAfter | F::IncompatibleWith => {
                if is_li(name) { Node::Li } else { Node::Other }
            }
            F::ModDependencies => {
                if is_li(name) {
                    about.mod_dependencies.push(Dependency::default());
                    Node::Li
                } else {
                    Node::Other
                }
            }
            F::ModDependenciesByVersion => {
                about.mod_dependencies_by_version.push((String::from_utf8_lossy(name).into_owned(), vec![]));
                Node::Ver(about.mod_dependencies_by_version.len() - 1)
            }
            F::LoadBeforeByVersion => {
                about.load_before_by_version.push((String::from_utf8_lossy(name).into_owned(), vec![]));
                Node::Ver(about.load_before_by_version.len() - 1)
            }
            F::LoadAfterByVersion => {
                about.load_after_by_version.push((String::from_utf8_lossy(name).into_owned(), vec![]));
                Node::Ver(about.load_after_by_version.len() - 1)
            }
            F::IncompatibleWithByVersion => {
                about.incompatible_with_by_version.push((String::from_utf8_lossy(name).into_owned(), vec![]));
                Node::Ver(about.incompatible_with_by_version.len() - 1)
            }
            F::DescriptionsByVersion => {
                about.descriptions_by_version.push((String::from_utf8_lossy(name).into_owned(), String::new()));
                Node::Ver(about.descriptions_by_version.len() - 1)
            }
            _ => Node::Other,
        },
        // <li> inside <v1.x> of a ByVersion list
        [Node::Root, Node::Top(f), Node::Ver(_)] => match f {
            F::ModDependenciesByVersion => {
                if is_li(name) {
                    if let Some((_, v)) = about.mod_dependencies_by_version.last_mut() {
                        v.push(Dependency::default());
                    }
                    Node::Li
                } else {
                    Node::Other
                }
            }
            F::LoadBeforeByVersion | F::LoadAfterByVersion | F::IncompatibleWithByVersion => {
                if is_li(name) { Node::Li } else { Node::Other }
            }
            _ => Node::Other,
        },
        // properties of a dependency <li>
        [Node::Root, Node::Top(F::ModDependencies), Node::Li] | [Node::Root, Node::Top(F::ModDependenciesByVersion), Node::Ver(_), Node::Li] => {
            Node::Dep(dep_field_of(name))
        }
        // <li> inside <alternativePackageIds>
        [.., Node::Li, Node::Dep(DF::AltIds)] => {
            if is_li(name) { Node::Li } else { Node::Other }
        }
        _ => Node::Other,
    }
}

fn last_dep<'a>(about: &'a mut About, stack: &[Node]) -> Option<&'a mut Dependency> {
    // `stack` here is the stack without the node being finished.
    for n in stack {
        if let Node::Top(F::ModDependenciesByVersion) = n {
            return about.mod_dependencies_by_version.last_mut().and_then(|(_, v)| v.last_mut());
        }
    }
    about.mod_dependencies.last_mut()
}

fn finish(about: &mut About, stack: &[Node], node: Node, buf: &str) {
    let t = buf.trim();
    match node {
        Node::Top(f) => {
            let s = || t.to_string();
            match f {
                F::PackageId => about.package_id = s(),
                F::Name => about.name = s(),
                F::ShortName => about.short_name = s(),
                F::Author => about.author = s(),
                F::Description => about.description = s(),
                F::Url => about.url = s(),
                F::ModIconPath => about.mod_icon_path = s(),
                F::ModVersion => about.mod_version = s(),
                F::SteamAppId => about.steam_app_id = s(),
                F::TargetVersion => about.target_version = s(),
                _ => {}
            }
        }
        Node::Ver(i) => {
            if let [Node::Root, Node::Top(F::DescriptionsByVersion)] = stack {
                if let Some(e) = about.descriptions_by_version.get_mut(i) {
                    e.1 = t.to_string();
                }
            }
        }
        Node::Li => match stack {
            [Node::Root, Node::Top(f)] => {
                let list = match f {
                    F::SupportedVersions => Some(&mut about.supported_versions),
                    F::Authors => Some(&mut about.authors),
                    F::LoadBefore => Some(&mut about.load_before),
                    F::LoadAfter => Some(&mut about.load_after),
                    F::ForceLoadBefore => Some(&mut about.force_load_before),
                    F::ForceLoadAfter => Some(&mut about.force_load_after),
                    F::IncompatibleWith => Some(&mut about.incompatible_with),
                    _ => None,
                };
                if let Some(l) = list {
                    l.push(t.to_string());
                }
            }
            [Node::Root, Node::Top(f), Node::Ver(_)] => {
                let list = match f {
                    F::LoadBeforeByVersion => about.load_before_by_version.last_mut(),
                    F::LoadAfterByVersion => about.load_after_by_version.last_mut(),
                    F::IncompatibleWithByVersion => about.incompatible_with_by_version.last_mut(),
                    _ => None,
                };
                if let Some((_, l)) = list {
                    l.push(t.to_string());
                }
            }
            [.., Node::Li, Node::Dep(DF::AltIds)] => {
                if let Some(d) = last_dep(about, stack) {
                    d.alternative_package_ids.push(t.to_string());
                }
            }
            _ => {}
        },
        Node::Dep(df) => {
            if let Some(d) = last_dep(about, stack) {
                match df {
                    DF::PackageId => d.package_id = t.to_string(),
                    DF::DisplayName => d.display_name = t.to_string(),
                    DF::SteamWorkshopUrl => d.steam_workshop_url = t.to_string(),
                    DF::DownloadUrl => d.download_url = t.to_string(),
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------------------------
// Variant B: quick-xml + serde derive. Case variants have to be listed as aliases.
// ---------------------------------------------------------------------------------------------

#[derive(Deserialize, Default)]
struct Li {
    #[serde(rename = "li", default)]
    li: Vec<String>,
}

#[derive(Deserialize, Default)]
struct DepS {
    #[serde(rename = "packageId", alias = "PackageId", alias = "packageid", default)]
    package_id: Option<String>,
    #[serde(rename = "displayName", alias = "DisplayName", alias = "displayname", default)]
    display_name: Option<String>,
    #[serde(rename = "steamWorkshopUrl", alias = "SteamWorkshopUrl", alias = "steamworkshopurl", default)]
    steam_workshop_url: Option<String>,
    #[serde(rename = "downloadUrl", alias = "DownloadUrl", alias = "downloadurl", default)]
    download_url: Option<String>,
    #[serde(rename = "alternativePackageIds", alias = "AlternativePackageIds", default)]
    alternative_package_ids: Option<Li>,
}

#[derive(Deserialize, Default)]
struct DepList {
    #[serde(rename = "li", default)]
    li: Vec<DepS>,
}

#[derive(Deserialize, Default)]
struct AboutS {
    #[serde(rename = "packageId", alias = "PackageId", alias = "packageid", default)]
    package_id: Option<String>,
    #[serde(rename = "name", alias = "Name", default)]
    name: Option<String>,
    #[serde(rename = "shortName", alias = "ShortName", alias = "shortname", default)]
    short_name: Option<String>,
    #[serde(rename = "author", alias = "Author", default)]
    author: Option<String>,
    #[serde(rename = "authors", alias = "Authors", default)]
    authors: Option<Li>,
    #[serde(rename = "description", alias = "Description", default)]
    description: Option<String>,
    #[serde(rename = "url", alias = "Url", alias = "URL", default)]
    url: Option<String>,
    #[serde(rename = "modIconPath", alias = "ModIconPath", alias = "modiconpath", default)]
    mod_icon_path: Option<String>,
    #[serde(rename = "modVersion", alias = "ModVersion", alias = "modversion", default)]
    mod_version: Option<String>,
    #[serde(rename = "steamAppId", alias = "SteamAppId", alias = "steamappid", default)]
    steam_app_id: Option<String>,
    #[serde(rename = "targetVersion", alias = "TargetVersion", alias = "targetversion", default)]
    target_version: Option<String>,
    #[serde(rename = "supportedVersions", alias = "SupportedVersions", alias = "supportedversions", default)]
    supported_versions: Option<Li>,
    #[serde(rename = "modDependencies", alias = "ModDependencies", alias = "moddependencies", default)]
    mod_dependencies: Option<DepList>,
    #[serde(rename = "loadBefore", alias = "LoadBefore", alias = "loadbefore", default)]
    load_before: Option<Li>,
    #[serde(rename = "loadAfter", alias = "LoadAfter", alias = "loadafter", default)]
    load_after: Option<Li>,
    #[serde(rename = "forceLoadBefore", alias = "ForceLoadBefore", alias = "forceloadbefore", default)]
    force_load_before: Option<Li>,
    #[serde(rename = "forceLoadAfter", alias = "ForceLoadAfter", alias = "forceloadafter", default)]
    force_load_after: Option<Li>,
    #[serde(rename = "incompatibleWith", alias = "IncompatibleWith", alias = "incompatiblewith", default)]
    incompatible_with: Option<Li>,
    #[serde(rename = "modDependenciesByVersion", alias = "ModDependenciesByVersion", alias = "moddependenciesbyversion", default)]
    mod_dependencies_by_version: Option<HashMap<String, DepList>>,
    #[serde(rename = "loadBeforeByVersion", alias = "LoadBeforeByVersion", alias = "loadbeforebyversion", default)]
    load_before_by_version: Option<HashMap<String, Li>>,
    #[serde(rename = "loadAfterByVersion", alias = "LoadAfterByVersion", alias = "loadafterbyversion", default)]
    load_after_by_version: Option<HashMap<String, Li>>,
    #[serde(rename = "incompatibleWithByVersion", alias = "IncompatibleWithByVersion", alias = "incompatiblewithbyversion", default)]
    incompatible_with_by_version: Option<HashMap<String, Li>>,
    #[serde(rename = "descriptionsByVersion", alias = "DescriptionsByVersion", alias = "descriptionsbyversion", default)]
    descriptions_by_version: Option<HashMap<String, String>>,
}

fn t(s: Option<String>) -> String {
    s.map(|s| s.trim().to_string()).unwrap_or_default()
}
fn tl(l: Option<Li>) -> Vec<String> {
    l.map(|l| l.li.into_iter().map(|s| s.trim().to_string()).collect()).unwrap_or_default()
}
fn td(d: DepS) -> Dependency {
    Dependency {
        package_id: t(d.package_id),
        display_name: t(d.display_name),
        steam_workshop_url: t(d.steam_workshop_url),
        download_url: t(d.download_url),
        alternative_package_ids: tl(d.alternative_package_ids),
    }
}
fn sorted_map<V>(m: Option<HashMap<String, V>>) -> Vec<(String, V)> {
    let mut v: Vec<(String, V)> = m.map(|m| m.into_iter().collect()).unwrap_or_default();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

pub fn parse_serde(text: &str) -> Result<About, String> {
    let s: AboutS = quick_xml::de::from_str(text).map_err(|e| e.to_string())?;
    Ok(About {
        package_id: t(s.package_id),
        name: t(s.name),
        short_name: t(s.short_name),
        author: t(s.author),
        authors: tl(s.authors),
        description: t(s.description),
        url: t(s.url),
        mod_icon_path: t(s.mod_icon_path),
        mod_version: t(s.mod_version),
        steam_app_id: t(s.steam_app_id),
        target_version: t(s.target_version),
        supported_versions: tl(s.supported_versions),
        mod_dependencies: s.mod_dependencies.map(|d| d.li.into_iter().map(td).collect()).unwrap_or_default(),
        load_before: tl(s.load_before),
        load_after: tl(s.load_after),
        force_load_before: tl(s.force_load_before),
        force_load_after: tl(s.force_load_after),
        incompatible_with: tl(s.incompatible_with),
        mod_dependencies_by_version: sorted_map(s.mod_dependencies_by_version).into_iter().map(|(k, v)| (k, v.li.into_iter().map(td).collect())).collect(),
        load_before_by_version: sorted_map(s.load_before_by_version).into_iter().map(|(k, v)| (k, tl(Some(v)))).collect(),
        load_after_by_version: sorted_map(s.load_after_by_version).into_iter().map(|(k, v)| (k, tl(Some(v)))).collect(),
        incompatible_with_by_version: sorted_map(s.incompatible_with_by_version).into_iter().map(|(k, v)| (k, tl(Some(v)))).collect(),
        descriptions_by_version: sorted_map(s.descriptions_by_version).into_iter().map(|(k, v)| (k, v.trim().to_string())).collect(),
    })
}

// ---------------------------------------------------------------------------------------------
// Variant C: roxmltree DOM
// ---------------------------------------------------------------------------------------------

fn node_text(n: roxmltree::Node) -> String {
    let mut s = String::new();
    for c in n.children() {
        if c.is_text() {
            s.push_str(c.text().unwrap_or(""));
        }
    }
    s.trim().to_string()
}

fn li_list(n: roxmltree::Node) -> Vec<String> {
    n.children().filter(|c| c.is_element() && is_li(c.tag_name().name().as_bytes())).map(node_text).collect()
}

fn dep_of(n: roxmltree::Node) -> Dependency {
    let mut d = Dependency::default();
    for c in n.children().filter(|c| c.is_element()) {
        match dep_field_of(c.tag_name().name().as_bytes()) {
            DF::PackageId => d.package_id = node_text(c),
            DF::DisplayName => d.display_name = node_text(c),
            DF::SteamWorkshopUrl => d.steam_workshop_url = node_text(c),
            DF::DownloadUrl => d.download_url = node_text(c),
            DF::AltIds => d.alternative_package_ids = li_list(c),
            DF::Unknown => {}
        }
    }
    d
}

fn dep_list(n: roxmltree::Node) -> Vec<Dependency> {
    n.children().filter(|c| c.is_element() && is_li(c.tag_name().name().as_bytes())).map(dep_of).collect()
}

pub fn parse_dom(text: &str) -> Result<About, String> {
    let opts = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let doc = roxmltree::Document::parse_with_options(text, opts).map_err(|e| e.to_string())?;
    let mut a = About::default();
    for c in doc.root_element().children().filter(|c| c.is_element()) {
        let name = c.tag_name().name().as_bytes();
        match field_of(name) {
            F::PackageId => a.package_id = node_text(c),
            F::Name => a.name = node_text(c),
            F::ShortName => a.short_name = node_text(c),
            F::Author => a.author = node_text(c),
            F::Authors => a.authors = li_list(c),
            F::Description => a.description = node_text(c),
            F::Url => a.url = node_text(c),
            F::ModIconPath => a.mod_icon_path = node_text(c),
            F::ModVersion => a.mod_version = node_text(c),
            F::SteamAppId => a.steam_app_id = node_text(c),
            F::TargetVersion => a.target_version = node_text(c),
            F::SupportedVersions => a.supported_versions = li_list(c),
            F::ModDependencies => a.mod_dependencies = dep_list(c),
            F::LoadBefore => a.load_before = li_list(c),
            F::LoadAfter => a.load_after = li_list(c),
            F::ForceLoadBefore => a.force_load_before = li_list(c),
            F::ForceLoadAfter => a.force_load_after = li_list(c),
            F::IncompatibleWith => a.incompatible_with = li_list(c),
            F::ModDependenciesByVersion => {
                for v in c.children().filter(|v| v.is_element()) {
                    a.mod_dependencies_by_version.push((v.tag_name().name().to_string(), dep_list(v)));
                }
            }
            F::LoadBeforeByVersion => {
                for v in c.children().filter(|v| v.is_element()) {
                    a.load_before_by_version.push((v.tag_name().name().to_string(), li_list(v)));
                }
            }
            F::LoadAfterByVersion => {
                for v in c.children().filter(|v| v.is_element()) {
                    a.load_after_by_version.push((v.tag_name().name().to_string(), li_list(v)));
                }
            }
            F::IncompatibleWithByVersion => {
                for v in c.children().filter(|v| v.is_element()) {
                    a.incompatible_with_by_version.push((v.tag_name().name().to_string(), li_list(v)));
                }
            }
            F::DescriptionsByVersion => {
                for v in c.children().filter(|v| v.is_element()) {
                    a.descriptions_by_version.push((v.tag_name().name().to_string(), node_text(v)));
                }
            }
            F::Unknown => {}
        }
    }
    // the serde variant returns by-version maps sorted by key; sort here so results are comparable
    a.mod_dependencies_by_version.sort_by(|x, y| x.0.cmp(&y.0));
    a.load_before_by_version.sort_by(|x, y| x.0.cmp(&y.0));
    a.load_after_by_version.sort_by(|x, y| x.0.cmp(&y.0));
    a.incompatible_with_by_version.sort_by(|x, y| x.0.cmp(&y.0));
    a.descriptions_by_version.sort_by(|x, y| x.0.cmp(&y.0));
    Ok(a)
}

/// Normalise the by-version vectors of the event parser (document order) to the sorted form of the others.
pub fn normalize(mut a: About) -> About {
    a.mod_dependencies_by_version.sort_by(|x, y| x.0.cmp(&y.0));
    a.load_before_by_version.sort_by(|x, y| x.0.cmp(&y.0));
    a.load_after_by_version.sort_by(|x, y| x.0.cmp(&y.0));
    a.incompatible_with_by_version.sort_by(|x, y| x.0.cmp(&y.0));
    a.descriptions_by_version.sort_by(|x, y| x.0.cmp(&y.0));
    a
}

/// Outcome classes used by the failure counters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Outcome {
    /// parsed without error
    Ok,
    /// parser reported an error but a partial result was produced (event parser only)
    Recovered,
    /// no result
    Failed,
}
