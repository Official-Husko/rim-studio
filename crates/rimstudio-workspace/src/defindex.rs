//! The def index: one cheap row per def, with paged search.
//!
//! The index is the always-on tier of the workspace. It is built from the raw files of the
//! reference set (abstract defs included, patches not applied), so it is available before and
//! independently of the def engine snapshot. Strings are interned: a def type or a package name
//! appears once however many defs use it.
//!
//! [`DefIndex::search`] filters by free text, def type and pack and returns one page. Without text
//! the order is the load order (pack, file, document order); with text the best matches come first
//! (exact `defName`, prefix, substring, then label matches) and load order breaks ties. The scan
//! is linear over interned rows; 13,000 defs take a few milliseconds.

use camino::Utf8PathBuf;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::snapshot::PackRef;

/// The default page size of a search.
pub const DEFAULT_PAGE_SIZE: usize = 200;

/// The raw facts about one def element of a Defs file.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefSummary {
    /// The def type: the `Class` attribute when the element has one, else the element name (for
    /// example `ThingDef`).
    pub def_type: String,
    /// The `defName`; empty for an abstract node that has only a `Name`.
    pub def_name: String,
    /// The `label`, when the node has one of its own.
    pub label: Option<String>,
    /// The `Name` attribute used by children to inherit.
    pub name: Option<String>,
    /// The `ParentName` attribute.
    pub parent: Option<String>,
    /// True when `Abstract="True"`.
    pub is_abstract: bool,
}

/// An interned string handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Sym(u32);

const NONE: Sym = Sym(u32::MAX);

#[derive(Debug, Default)]
struct Interner {
    map: FxHashMap<Arc<str>, Sym>,
    strings: Vec<Arc<str>>,
}

impl Interner {
    fn intern(&mut self, text: &str) -> Sym {
        if let Some(&s) = self.map.get(text) {
            return s;
        }
        let sym = Sym(u32::try_from(self.strings.len()).unwrap_or(u32::MAX - 1));
        let rc: Arc<str> = Arc::from(text);
        self.strings.push(Arc::clone(&rc));
        self.map.insert(rc, sym);
        sym
    }

    fn intern_opt(&mut self, text: Option<&str>) -> Sym {
        match text {
            Some(t) if !t.is_empty() => self.intern(t),
            _ => NONE,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Entry {
    def_type: Sym,
    def_name: Sym,
    label: Sym,
    name: Sym,
    parent: Sym,
    search: Sym,
    pack: u32,
    file: u32,
    is_abstract: bool,
}

#[derive(Debug, Clone)]
struct IndexedFile {
    pack: u32,
    relative: Sym,
    path: Utf8PathBuf,
}

/// Collects defs and builds a [`DefIndex`].
#[derive(Debug)]
pub struct DefIndexBuilder {
    interner: Interner,
    entries: Vec<Entry>,
    packs: Vec<PackRef>,
    files: Vec<IndexedFile>,
}

impl DefIndexBuilder {
    /// A builder for the given packs (the pack of a file is its position in this list).
    #[must_use]
    pub fn new(packs: Vec<PackRef>) -> Self {
        DefIndexBuilder {
            interner: Interner::default(),
            entries: Vec::new(),
            packs,
            files: Vec::new(),
        }
    }

    /// Registers a file and returns its handle. Handles are dense from zero, in call order, so a
    /// caller that registers files in its own file table order gets the same numbers.
    pub fn add_file(&mut self, pack: usize, relative: &str, path: &camino::Utf8Path) -> u32 {
        let relative = self.interner.intern(relative);
        self.files.push(IndexedFile {
            pack: u32::try_from(pack).unwrap_or(u32::MAX),
            relative,
            path: path.to_owned(),
        });
        u32::try_from(self.files.len().saturating_sub(1)).unwrap_or(u32::MAX)
    }

    /// Adds one def of a registered file. A summary with neither `defName` nor `Name` is ignored.
    pub fn add_def(&mut self, file: u32, def: &DefSummary) {
        if def.def_name.is_empty() && def.name.as_deref().is_none_or(str::is_empty) {
            return;
        }
        let Some(f) = self.files.get(file as usize) else {
            return;
        };
        let pack = f.pack;
        let mut hay = String::with_capacity(def.def_name.len() + 16);
        hay.push_str(&def.def_name);
        hay.push('\n');
        if let Some(l) = &def.label {
            hay.push_str(l);
        }
        hay.push('\n');
        if let Some(n) = &def.name {
            hay.push_str(n);
        }
        let hay = hay.to_lowercase();
        let entry = Entry {
            def_type: self.interner.intern(&def.def_type),
            def_name: self.interner.intern_opt(Some(&def.def_name)),
            label: self.interner.intern_opt(def.label.as_deref()),
            name: self.interner.intern_opt(def.name.as_deref()),
            parent: self.interner.intern_opt(def.parent.as_deref()),
            search: self.interner.intern(&hay),
            pack,
            file,
            is_abstract: def.is_abstract,
        };
        self.entries.push(entry);
    }

    /// Finishes the index.
    #[must_use]
    pub fn build(self) -> DefIndex {
        let DefIndexBuilder {
            interner,
            entries,
            packs,
            files,
        } = self;
        let mut by_name: FxHashMap<u32, Vec<u32>> = FxHashMap::default();
        let mut type_counts: FxHashMap<u32, u32> = FxHashMap::default();
        for (i, e) in entries.iter().enumerate() {
            if e.def_name != NONE {
                by_name
                    .entry(e.def_name.0)
                    .or_default()
                    .push(u32::try_from(i).unwrap_or(u32::MAX));
            }
            *type_counts.entry(e.def_type.0).or_insert(0) += 1;
        }
        let mut types: Vec<(Sym, u32)> =
            type_counts.into_iter().map(|(s, c)| (Sym(s), c)).collect();
        types.sort_by(|a, b| {
            let (x, y) = (
                interner.strings.get(a.0.0 as usize),
                interner.strings.get(b.0.0 as usize),
            );
            x.cmp(&y)
        });
        DefIndex {
            strings: interner.strings,
            entries,
            packs,
            files,
            by_name,
            types,
        }
    }
}

/// How the hits of a [`Query`] are ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Sort {
    /// Best text match first when there is text, otherwise load order.
    #[default]
    Relevance,
    /// By `defName` (case insensitive), then type, then load order.
    Name,
}

/// A def search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Query {
    /// Words that must all appear in the `defName`, the label or the `Name` (case insensitive).
    pub text: String,
    /// Only defs of this type (the element name, case insensitive).
    pub def_type: Option<String>,
    /// Only defs of this pack (package id, case insensitive).
    pub pack: Option<String>,
    /// Include abstract nodes.
    pub include_abstract: bool,
    /// The order of the hits.
    pub sort: Sort,
}

impl Default for Query {
    fn default() -> Self {
        Query {
            text: String::new(),
            def_type: None,
            pack: None,
            include_abstract: true,
            sort: Sort::Relevance,
        }
    }
}

impl Query {
    /// A text query with every other option at its default.
    #[must_use]
    pub fn text(text: impl Into<String>) -> Self {
        Query {
            text: text.into(),
            ..Query::default()
        }
    }

    /// Restricts the query to one def type.
    #[must_use]
    pub fn of_type(mut self, def_type: impl Into<String>) -> Self {
        self.def_type = Some(def_type.into());
        self
    }

    /// Restricts the query to one pack by package id.
    #[must_use]
    pub fn in_pack(mut self, package_id: impl Into<String>) -> Self {
        self.pack = Some(package_id.into());
        self
    }

    /// Leaves abstract nodes out.
    #[must_use]
    pub fn concrete_only(mut self) -> Self {
        self.include_abstract = false;
        self
    }
}

/// One page of a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    /// The number of hits to skip.
    pub offset: usize,
    /// The most hits to return.
    pub limit: usize,
}

impl Default for Page {
    fn default() -> Self {
        Page {
            offset: 0,
            limit: DEFAULT_PAGE_SIZE,
        }
    }
}

impl Page {
    /// A page of `limit` hits starting at `offset`.
    #[must_use]
    pub fn new(offset: usize, limit: usize) -> Self {
        Page { offset, limit }
    }
}

/// One def found by a search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DefHit {
    /// The def type (the element name).
    pub def_type: String,
    /// The `defName` (empty for an abstract node that has only a `Name`).
    pub def_name: String,
    /// The `label` the node itself declares.
    pub label: Option<String>,
    /// The `Name` attribute.
    pub name: Option<String>,
    /// The `ParentName` attribute.
    pub parent: Option<String>,
    /// True for an abstract node.
    pub is_abstract: bool,
    /// The pack that owns the file.
    pub pack: PackRef,
    /// The file handle (an index into the session's file table).
    pub file: u32,
    /// The file path relative to its load folder, `Defs/...` style.
    pub relative: String,
    /// The absolute file path.
    pub path: Utf8PathBuf,
}

/// The result of a search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageResult {
    /// The number of defs that match, over all pages.
    pub total: usize,
    /// The offset of the first hit of this page.
    pub offset: usize,
    /// The hits of this page.
    pub hits: Vec<DefHit>,
}

/// A def type with the number of nodes of that type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeCount {
    /// The def type (the element name).
    pub def_type: String,
    /// The number of nodes, abstract ones included.
    pub count: usize,
}

/// The def index of a workspace session.
#[derive(Debug, Clone)]
pub struct DefIndex {
    strings: Vec<Arc<str>>,
    entries: Vec<Entry>,
    packs: Vec<PackRef>,
    files: Vec<IndexedFile>,
    by_name: FxHashMap<u32, Vec<u32>>,
    types: Vec<(Sym, u32)>,
}

impl DefIndex {
    /// An index with no defs.
    #[must_use]
    pub fn empty() -> Self {
        DefIndexBuilder::new(Vec::new()).build()
    }

    fn s(&self, sym: Sym) -> &str {
        self.strings.get(sym.0 as usize).map_or("", |s| &**s)
    }

    fn opt(&self, sym: Sym) -> Option<String> {
        (sym != NONE).then(|| self.s(sym).to_owned())
    }

    /// The number of indexed nodes, abstract ones included.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// True when the index holds no node.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The number of abstract nodes.
    #[must_use]
    pub fn abstract_count(&self) -> usize {
        self.entries.iter().filter(|e| e.is_abstract).count()
    }

    /// The packs of the index in load order.
    #[must_use]
    pub fn packs(&self) -> &[PackRef] {
        &self.packs
    }

    /// The def types with their node counts, sorted by name.
    #[must_use]
    pub fn types(&self) -> Vec<TypeCount> {
        self.types
            .iter()
            .map(|(s, c)| TypeCount {
                def_type: self.s(*s).to_owned(),
                count: *c as usize,
            })
            .collect()
    }

    /// The number of nodes per pack, in pack order.
    #[must_use]
    pub fn pack_counts(&self) -> Vec<usize> {
        let mut counts = vec![0usize; self.packs.len()];
        for e in &self.entries {
            if let Some(c) = counts.get_mut(e.pack as usize) {
                *c += 1;
            }
        }
        counts
    }

    fn hit(&self, i: usize) -> Option<DefHit> {
        let e = self.entries.get(i)?;
        let file = self.files.get(e.file as usize)?;
        let pack = self.packs.get(e.pack as usize)?.clone();
        Some(DefHit {
            def_type: self.s(e.def_type).to_owned(),
            def_name: self.s(e.def_name).to_owned(),
            label: self.opt(e.label),
            name: self.opt(e.name),
            parent: self.opt(e.parent),
            is_abstract: e.is_abstract,
            pack,
            file: e.file,
            relative: self.s(file.relative).to_owned(),
            path: file.path.clone(),
        })
    }

    /// Every node with this `defName`, in load order, optionally of one type (case insensitive).
    #[must_use]
    pub fn find(&self, def_type: Option<&str>, def_name: &str) -> Vec<DefHit> {
        let Some(sym) = self.strings.iter().position(|s| &**s == def_name) else {
            return Vec::new();
        };
        let Some(list) = u32::try_from(sym).ok().and_then(|s| self.by_name.get(&s)) else {
            return Vec::new();
        };
        list.iter()
            .filter(|&&i| {
                def_type.is_none_or(|t| {
                    self.entries
                        .get(i as usize)
                        .is_some_and(|e| self.s(e.def_type).eq_ignore_ascii_case(t))
                })
            })
            .filter_map(|&i| self.hit(i as usize))
            .collect()
    }

    /// Searches the index and returns one page.
    #[must_use]
    pub fn search(&self, query: &Query, page: Page) -> PageResult {
        let terms: Vec<String> = query
            .text
            .split_whitespace()
            .map(str::to_lowercase)
            .collect();
        let type_syms: Option<Vec<Sym>> = query.def_type.as_ref().map(|t| {
            self.types
                .iter()
                .map(|(s, _)| *s)
                .filter(|s| self.s(*s).eq_ignore_ascii_case(t.trim()))
                .collect()
        });
        let pack_set: Option<Vec<u32>> = query.pack.as_ref().map(|p| {
            self.packs
                .iter()
                .filter(|r| r.package_id.eq_ignore_ascii_case(p.trim()))
                .filter_map(|r| u32::try_from(r.index).ok())
                .collect()
        });
        let matches = |e: &Entry| -> bool {
            if !query.include_abstract && e.is_abstract {
                return false;
            }
            if let Some(types) = &type_syms
                && !types.contains(&e.def_type)
            {
                return false;
            }
            if let Some(packs) = &pack_set
                && !packs.contains(&e.pack)
            {
                return false;
            }
            if terms.is_empty() {
                return true;
            }
            let hay = self.s(e.search);
            terms.iter().all(|t| hay.contains(t.as_str()))
        };

        let ordered = !terms.is_empty() || query.sort == Sort::Name;
        if !ordered {
            // load order: count everything, materialise only the page
            let mut total = 0usize;
            let mut hits = Vec::new();
            for (i, e) in self.entries.iter().enumerate() {
                if !matches(e) {
                    continue;
                }
                if total >= page.offset && hits.len() < page.limit {
                    hits.extend(self.hit(i));
                }
                total += 1;
            }
            return PageResult {
                total,
                offset: page.offset,
                hits,
            };
        }

        let first = terms.first().map(String::as_str);
        let mut found: Vec<(u8, usize)> = self
            .entries
            .iter()
            .enumerate()
            .filter(|(_, e)| matches(e))
            .map(|(i, e)| {
                let score = match (query.sort, first) {
                    (Sort::Relevance, Some(t)) => {
                        let name = self.s(e.def_name).to_lowercase();
                        if name == t {
                            0
                        } else if name.starts_with(t) {
                            1
                        } else if name.contains(t) {
                            2
                        } else {
                            3
                        }
                    }
                    _ => 0,
                };
                (score, i)
            })
            .collect();
        if query.sort == Sort::Name {
            found.sort_by(|a, b| {
                let (ea, eb) = (self.entries.get(a.1), self.entries.get(b.1));
                let key = |e: Option<&Entry>| {
                    e.map(|e| {
                        (
                            self.s(e.def_name).to_lowercase(),
                            self.s(e.def_type).to_owned(),
                        )
                    })
                };
                key(ea).cmp(&key(eb)).then(a.1.cmp(&b.1))
            });
        } else {
            found.sort();
        }
        let total = found.len();
        let hits = found
            .into_iter()
            .skip(page.offset)
            .take(page.limit)
            .filter_map(|(_, i)| self.hit(i))
            .collect();
        PageResult {
            total,
            offset: page.offset,
            hits,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refset::PackKind;
    use camino::Utf8Path;

    fn packs() -> Vec<PackRef> {
        vec![
            PackRef {
                index: 0,
                package_id: "ludeon.rimworld".into(),
                name: "Core".into(),
                kind: PackKind::Core,
            },
            PackRef {
                index: 1,
                package_id: "rs.mod".into(),
                name: "RS Mod".into(),
                kind: PackKind::Mod,
            },
        ]
    }

    fn def(t: &str, name: &str, label: Option<&str>) -> DefSummary {
        DefSummary {
            def_type: t.into(),
            def_name: name.into(),
            label: label.map(str::to_owned),
            ..DefSummary::default()
        }
    }

    fn sample() -> DefIndex {
        let mut b = DefIndexBuilder::new(packs());
        let f0 = b.add_file(0, "Defs/core.xml", Utf8Path::new("/g/Core/Defs/core.xml"));
        let f1 = b.add_file(1, "Defs/mod.xml", Utf8Path::new("/m/Defs/mod.xml"));
        b.add_def(f0, &def("ThingDef", "RS_Rifle", Some("test rifle")));
        b.add_def(f0, &def("ThingDef", "RS_RifleAmmo", Some("rifle ammo")));
        b.add_def(f0, &def("RS_RecipeDef", "RS_Make", Some("make a rifle")));
        let mut abs = def("ThingDef", "", None);
        abs.name = Some("RS_BaseGun".into());
        abs.is_abstract = true;
        b.add_def(f0, &abs);
        b.add_def(f1, &def("ThingDef", "RS_Pistol", Some("test pistol")));
        b.add_def(f1, &def("ThingDef", "RS_Rifle", Some("modded rifle")));
        b.add_def(f1, &DefSummary::default());
        b.build()
    }

    fn names(r: &PageResult) -> Vec<String> {
        r.hits.iter().map(|h| h.def_name.clone()).collect()
    }

    #[test]
    fn empty_query_lists_everything_in_load_order() {
        let index = sample();
        let r = index.search(&Query::default(), Page::default());
        assert_eq!(r.total, 6);
        assert_eq!(
            names(&r),
            vec![
                "RS_Rifle",
                "RS_RifleAmmo",
                "RS_Make",
                "",
                "RS_Pistol",
                "RS_Rifle"
            ]
        );
        assert_eq!(index.abstract_count(), 1);
    }

    #[test]
    fn text_matches_name_and_label_with_exact_first() {
        let index = sample();
        let r = index.search(&Query::text("rifle"), Page::default());
        assert_eq!(r.total, 4);
        assert_eq!(
            names(&r),
            vec!["RS_Rifle", "RS_RifleAmmo", "RS_Rifle", "RS_Make"]
        );
        let r = index.search(&Query::text("TEST pistol"), Page::default());
        assert_eq!(names(&r), vec!["RS_Pistol"]);
        let r = index.search(&Query::text("rs_rifle"), Page::default());
        assert_eq!(
            names(&r)[..2],
            ["RS_Rifle".to_owned(), "RS_Rifle".to_owned()]
        );
    }

    #[test]
    fn filters_by_type_pack_and_abstract() {
        let index = sample();
        let r = index.search(&Query::default().of_type("rs_recipedef"), Page::default());
        assert_eq!(names(&r), vec!["RS_Make"]);
        let r = index.search(&Query::default().in_pack("RS.MOD"), Page::default());
        assert_eq!(r.total, 2);
        assert_eq!(r.hits[0].pack.package_id, "rs.mod");
        assert_eq!(r.hits[0].relative, "Defs/mod.xml");
        let r = index.search(&Query::default().concrete_only(), Page::default());
        assert_eq!(r.total, 5);
        let r = index.search(&Query::default().in_pack("nobody"), Page::default());
        assert_eq!(r.total, 0);
    }

    #[test]
    fn paging_slices_the_ordered_result() {
        let index = sample();
        let all = index.search(&Query::default(), Page::default());
        let p = index.search(&Query::default(), Page::new(2, 2));
        assert_eq!(p.total, 6);
        assert_eq!(p.offset, 2);
        assert_eq!(p.hits, all.hits[2..4].to_vec());
        let past = index.search(&Query::default(), Page::new(10, 5));
        assert!(past.hits.is_empty());
        assert_eq!(past.total, 6);
        let zero = index.search(&Query::text("rifle"), Page::new(0, 0));
        assert!(zero.hits.is_empty());
        assert_eq!(zero.total, 4);
    }

    #[test]
    fn name_sort_is_case_insensitive_and_stable() {
        let index = sample();
        let r = index.search(
            &Query {
                sort: Sort::Name,
                ..Query::default()
            },
            Page::default(),
        );
        assert_eq!(
            names(&r),
            vec![
                "",
                "RS_Make",
                "RS_Pistol",
                "RS_Rifle",
                "RS_Rifle",
                "RS_RifleAmmo"
            ]
        );
        // the first RS_Rifle is the Core one (load order breaks the tie)
        assert_eq!(r.hits[3].pack.package_id, "ludeon.rimworld");
    }

    #[test]
    fn find_and_types_and_counts() {
        let index = sample();
        let found = index.find(Some("thingdef"), "RS_Rifle");
        assert_eq!(found.len(), 2);
        assert!(index.find(Some("RS_RecipeDef"), "RS_Rifle").is_empty());
        assert!(index.find(None, "RS_Nothing").is_empty());
        let types = index.types();
        assert_eq!(
            types,
            vec![
                TypeCount {
                    def_type: "RS_RecipeDef".into(),
                    count: 1
                },
                TypeCount {
                    def_type: "ThingDef".into(),
                    count: 5
                },
            ]
        );
        assert_eq!(index.pack_counts(), vec![4, 2]);
        assert_eq!(index.len(), 6);
        assert!(!index.is_empty());
        assert!(DefIndex::empty().is_empty());
    }

    #[test]
    fn search_over_13000_defs_is_fast_enough() {
        let mut b = DefIndexBuilder::new(packs());
        let f = b.add_file(0, "Defs/big.xml", Utf8Path::new("/g/Defs/big.xml"));
        for i in 0..13_000 {
            let t = if i % 7 == 0 {
                "RS_RecipeDef"
            } else {
                "ThingDef"
            };
            b.add_def(
                f,
                &def(
                    t,
                    &format!("RS_Item{i}"),
                    Some(&format!("fictional item number {i}")),
                ),
            );
        }
        let index = b.build();
        let started = std::time::Instant::now();
        let r = index.search(&Query::text("item 12"), Page::default());
        let r2 = index.search(&Query::default().of_type("ThingDef"), Page::new(5000, 200));
        let elapsed = started.elapsed();
        assert!(r.total > 100);
        assert_eq!(r2.hits.len(), 200);
        assert!(
            elapsed < std::time::Duration::from_millis(250),
            "two searches took {elapsed:?}"
        );
    }

    mod properties {
        use super::*;
        use proptest::prelude::*;

        type Row = (usize, String, Option<String>, bool, usize);

        fn arb_rows() -> impl Strategy<Value = Vec<Row>> {
            proptest::collection::vec(
                (
                    0usize..3,
                    "[a-cA-C_]{1,5}",
                    proptest::option::of("[a-c ]{0,6}"),
                    any::<bool>(),
                    0usize..2,
                ),
                0..40,
            )
        }

        proptest! {
            #[test]
            fn search_agrees_with_a_brute_force_filter(
                rows in arb_rows(),
                text in "[a-c ]{0,4}",
                only_type in proptest::option::of(0usize..3),
                concrete in any::<bool>(),
                offset in 0usize..5,
                limit in 0usize..8,
            ) {
                let types = ["ThingDef", "RS_RecipeDef", "RS_Other"];
                let mut b = DefIndexBuilder::new(packs());
                let f0 = b.add_file(0, "Defs/a.xml", Utf8Path::new("/a.xml"));
                let f1 = b.add_file(1, "Defs/b.xml", Utf8Path::new("/b.xml"));
                let mut model: Vec<(String, String, Option<String>, bool)> = Vec::new();
                for (t, name, label, abs, pack) in &rows {
                    let mut d = def(types[*t], name, label.as_deref());
                    d.is_abstract = *abs;
                    b.add_def(if *pack == 0 { f0 } else { f1 }, &d);
                    model.push((types[*t].to_owned(), name.clone(), label.clone(), *abs));
                }
                let index = b.build();
                let mut q = Query::text(text.clone());
                q.include_abstract = !concrete;
                q.def_type = only_type.map(|t| types[t].to_owned());
                let terms: Vec<String> = text.split_whitespace().map(str::to_lowercase).collect();
                let expected: Vec<&(String, String, Option<String>, bool)> = model
                    .iter()
                    .filter(|(t, name, label, abs)| {
                        let hay = format!("{name}\n{}\n", label.clone().unwrap_or_default()).to_lowercase();
                        (!concrete || !*abs)
                            && only_type.is_none_or(|o| types[o] == t)
                            && terms.iter().all(|term| hay.contains(term.as_str()))
                    })
                    .collect();
                let all = index.search(&q, Page::new(0, usize::MAX));
                prop_assert_eq!(all.total, expected.len());
                prop_assert_eq!(all.hits.len(), expected.len());
                let mut got: Vec<(String, String)> =
                    all.hits.iter().map(|h| (h.def_type.clone(), h.def_name.clone())).collect();
                let mut want: Vec<(String, String)> =
                    expected.iter().map(|(t, n, _, _)| (t.clone(), n.clone())).collect();
                got.sort();
                want.sort();
                prop_assert_eq!(got, want);
                // a page is a slice of the full ordered result
                let page = index.search(&q, Page::new(offset, limit));
                prop_assert_eq!(page.total, all.total);
                let slice: Vec<&DefHit> = all.hits.iter().skip(offset).take(limit).collect();
                prop_assert_eq!(page.hits.iter().collect::<Vec<_>>(), slice);
            }
        }
    }
}
