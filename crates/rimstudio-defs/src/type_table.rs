//! The def type table: which element names and `Class` values are def types.
//!
//! The game resolves a type name through `GenTypes.GetTypeInAnyAssembly`: a case sensitive index of
//! the short names of types in a few "ignored" namespaces, then a case insensitive lookup of full
//! names, then the ignored namespaces prefixed to the name. [`TypeTable`] reproduces that search
//! over a table of the def classes of the loaded assemblies.
//!
//! The table is data (JSON, never compiled in). Its shape is the one of the research prototype:
//!
//! ```json
//! {
//!   "format": 1,
//!   "root": "Verse.Def",
//!   "assemblies": ["Assembly-CSharp"],
//!   "types": { "Verse.ThingDef": { "base": "Verse.Def", "abstract": false, "assembly": "Assembly-CSharp" } },
//!   "short_name_collisions": {}
//! }
//! ```
//!
//! The order of `types` is the assembly load order: for a short name that appears twice the type
//! found last wins, for a case insensitive full name the first wins (as in the game). The real
//! table is generated at run time from the user's assemblies; repository tests use a small
//! fictional one.

use std::collections::BTreeMap;
use std::fmt;

use rustc_hash::FxHashMap;
use serde::de::{MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::error::{DefsError, DefsResult};

/// The namespaces whose types the game finds by short name.
pub const IGNORED_NAMESPACES: [&str; 12] = [
    "RimWorld",
    "Verse",
    "LudeonTK",
    "Verse.AI",
    "Verse.AI.Group",
    "Verse.Sound",
    "Verse.Grammar",
    "RimWorld.Planet",
    "RimWorld.BaseGen",
    "RimWorld.QuestGen",
    "RimWorld.SketchGen",
    "System",
];

/// The root def type of the game.
pub const DEFAULT_ROOT: &str = "Verse.Def";

/// What the table knows about one type.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct TypeInfo {
    /// The full name of the base type, `None` for a root.
    #[serde(default)]
    pub base: Option<String>,
    /// True for abstract classes.
    #[serde(default, rename = "abstract")]
    pub is_abstract: bool,
    /// The assembly that declares the type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assembly: Option<String>,
}

impl TypeInfo {
    /// A concrete type with the given base.
    pub fn with_base(base: impl Into<String>) -> Self {
        Self {
            base: Some(base.into()),
            is_abstract: false,
            assembly: None,
        }
    }
}

/// An insertion ordered JSON object of types.
#[derive(Debug, Clone, Default)]
struct OrderedTypes(Vec<(String, TypeInfo)>);

impl<'de> Deserialize<'de> for OrderedTypes {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = OrderedTypes;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object mapping full type names to type info")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<OrderedTypes, A::Error> {
                let mut out = Vec::with_capacity(map.size_hint().unwrap_or(0));
                while let Some((k, v)) = map.next_entry::<String, TypeInfo>()? {
                    out.push((k, v));
                }
                Ok(OrderedTypes(out))
            }
        }
        d.deserialize_map(V)
    }
}

impl Serialize for OrderedTypes {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(Some(self.0.len()))?;
        for (k, v) in &self.0 {
            m.serialize_entry(k, v)?;
        }
        m.end()
    }
}

fn default_format() -> u32 {
    1
}

fn default_root() -> String {
    DEFAULT_ROOT.to_owned()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawTable {
    #[serde(default = "default_format")]
    format: u32,
    #[serde(default = "default_root")]
    root: String,
    #[serde(default)]
    assemblies: Vec<String>,
    types: OrderedTypes,
    #[serde(default)]
    short_name_collisions: BTreeMap<String, Vec<String>>,
}

/// The def classes of the loaded assemblies and the game's type name search.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "RawTable", into = "RawTable")]
pub struct TypeTable {
    format: u32,
    root: String,
    assemblies: Vec<String>,
    types: Vec<(String, TypeInfo)>,
    collisions: BTreeMap<String, Vec<String>>,
    by_full: FxHashMap<String, usize>,
    by_short: FxHashMap<String, usize>,
    by_lower_full: FxHashMap<String, usize>,
    ancestors: Vec<Vec<u32>>,
}

impl TryFrom<RawTable> for TypeTable {
    type Error = DefsError;

    fn try_from(raw: RawTable) -> Result<Self, DefsError> {
        TypeTable::build(
            raw.format,
            raw.root,
            raw.assemblies,
            raw.types.0,
            raw.short_name_collisions,
        )
    }
}

impl From<TypeTable> for RawTable {
    fn from(t: TypeTable) -> Self {
        RawTable {
            format: t.format,
            root: t.root,
            assemblies: t.assemblies,
            types: OrderedTypes(t.types),
            short_name_collisions: t.collisions,
        }
    }
}

/// The namespace and short name of a full type name (`Ns.Outer/Inner` and the reflection spelling
/// `Ns.Outer+Inner` both give `Ns` and `Inner`).
fn split_name(full: &str) -> (&str, &str) {
    let outer = full.split(['/', '+']).next().unwrap_or(full);
    let ns = if outer.contains('.') {
        outer.rsplit_once('.').map_or("", |(ns, _)| ns)
    } else {
        ""
    };
    let after_slash = full.rsplit(['/', '+']).next().unwrap_or(full);
    let short = after_slash.rsplit('.').next().unwrap_or(after_slash);
    (ns, short)
}

impl TypeTable {
    /// Builds a table from types in assembly load order, with [`DEFAULT_ROOT`] as the root.
    ///
    /// # Errors
    /// [`DefsError::InvalidTypeTable`] for a repeated full name or an empty name.
    pub fn new(types: Vec<(String, TypeInfo)>) -> DefsResult<Self> {
        Self::build(1, default_root(), Vec::new(), types, BTreeMap::new())
    }

    /// Builds a table from its parts: the root, the assembly names, the types in assembly load
    /// order and the short name collisions (see the module documentation for their meaning).
    ///
    /// # Errors
    /// [`DefsError::InvalidTypeTable`] for a repeated full name or an empty name.
    pub fn from_parts(
        root: String,
        assemblies: Vec<String>,
        types: Vec<(String, TypeInfo)>,
        collisions: BTreeMap<String, Vec<String>>,
    ) -> DefsResult<Self> {
        Self::build(1, root, assemblies, types, collisions)
    }

    fn build(
        format: u32,
        root: String,
        assemblies: Vec<String>,
        types: Vec<(String, TypeInfo)>,
        collisions: BTreeMap<String, Vec<String>>,
    ) -> DefsResult<Self> {
        let mut by_full = FxHashMap::default();
        let mut by_short = FxHashMap::default();
        let mut by_lower_full = FxHashMap::default();
        for (i, (full, _)) in types.iter().enumerate() {
            if full.is_empty() {
                return Err(DefsError::InvalidTypeTable {
                    message: "a type has an empty name".to_owned(),
                });
            }
            if by_full.insert(full.clone(), i).is_some() {
                return Err(DefsError::InvalidTypeTable {
                    message: format!("type {full} is listed twice"),
                });
            }
            let (ns, short) = split_name(full);
            if ns.is_empty() || IGNORED_NAMESPACES.contains(&ns) {
                by_short.insert(short.to_owned(), i);
            }
            by_lower_full.entry(full.to_lowercase()).or_insert(i);
        }
        for (short, names) in &collisions {
            if let Some(&i) = names.last().and_then(|n| by_full.get(n)) {
                by_short.insert(short.clone(), i);
            }
        }
        let ancestors = (0..types.len())
            .map(|i| chain(&types, &by_full, &root, i))
            .collect();
        Ok(Self {
            format,
            root,
            assemblies,
            types,
            collisions,
            by_full,
            by_short,
            by_lower_full,
            ancestors,
        })
    }

    /// Parses a table from JSON text.
    ///
    /// # Errors
    /// [`DefsError::InvalidTypeTable`] for malformed JSON or an inconsistent table.
    pub fn from_json_str(json: &str) -> DefsResult<Self> {
        serde_json::from_str(json).map_err(|e| DefsError::InvalidTypeTable {
            message: e.to_string(),
        })
    }

    /// Parses a table from a JSON value.
    ///
    /// # Errors
    /// See [`TypeTable::from_json_str`].
    pub fn from_json_value(value: serde_json::Value) -> DefsResult<Self> {
        serde_json::from_value(value).map_err(|e| DefsError::InvalidTypeTable {
            message: e.to_string(),
        })
    }

    /// The table as a JSON value in the file shape.
    #[must_use]
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap_or(serde_json::Value::Null)
    }

    /// The full name of the root def type (`Verse.Def`).
    #[must_use]
    pub fn root(&self) -> &str {
        &self.root
    }

    /// Number of types.
    #[must_use]
    pub fn len(&self) -> usize {
        self.types.len()
    }

    /// True when the table has no type.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }

    /// The types in table order, with their full names.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &TypeInfo)> {
        self.types.iter().map(|(k, v)| (k.as_str(), v))
    }

    /// What the table says about a type, by full name.
    #[must_use]
    pub fn info(&self, full: &str) -> Option<&TypeInfo> {
        self.by_full
            .get(full)
            .and_then(|&i| self.types.get(i))
            .map(|(_, info)| info)
    }

    /// The position of a type by full name.
    pub(crate) fn type_id(&self, full: &str) -> Option<usize> {
        self.by_full.get(full).copied()
    }

    /// The full name of the type at a position.
    pub(crate) fn name_at(&self, id: usize) -> Option<&str> {
        self.types.get(id).map(|(k, _)| k.as_str())
    }

    /// The ids of a type and its ancestors (nearest first) that exist in the table.
    pub(crate) fn ancestor_ids(&self, id: usize) -> &[u32] {
        self.ancestors.get(id).map_or(&[], Vec::as_slice)
    }

    /// The game's type search: the full name of the def type that `name` denotes, or `None` when
    /// the name is unknown or the type is no def type.
    #[must_use]
    pub fn lookup(&self, name: &str) -> Option<&str> {
        self.lookup_id(name).and_then(|i| self.name_at(i))
    }

    pub(crate) fn lookup_id(&self, name: &str) -> Option<usize> {
        if let Some(&i) = self.by_short.get(name) {
            return Some(i);
        }
        let lower = name.to_lowercase();
        if let Some(&i) = self.by_lower_full.get(&lower) {
            return Some(i);
        }
        for ns in IGNORED_NAMESPACES {
            let key = format!("{ns}.{name}").to_lowercase();
            if let Some(&i) = self.by_lower_full.get(&key) {
                return Some(i);
            }
        }
        None
    }

    /// The type, its base, and so on up to and including the root, by full name. Names that the
    /// table does not list are not included.
    #[must_use]
    pub fn ancestors(&self, full: &str) -> Vec<&str> {
        match self.type_id(full) {
            Some(i) => self
                .ancestor_ids(i)
                .iter()
                .filter_map(|&a| self.name_at(a as usize))
                .collect(),
            None => Vec::new(),
        }
    }

    /// True when `full` is `ancestor` or derives from it.
    #[must_use]
    pub fn is_a(&self, full: &str, ancestor: &str) -> bool {
        self.ancestors(full).contains(&ancestor)
    }
}

/// The chain of table positions from `start` up to the root (a cycle stops the walk).
fn chain(
    types: &[(String, TypeInfo)],
    by_full: &FxHashMap<String, usize>,
    root: &str,
    start: usize,
) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    let mut cur = Some(start);
    while let Some(i) = cur {
        let Ok(id) = u32::try_from(i) else { break };
        if out.contains(&id) {
            break;
        }
        out.push(id);
        let Some((name, info)) = types.get(i) else {
            break;
        };
        if name == root {
            break;
        }
        cur = info.base.as_ref().and_then(|b| by_full.get(b)).copied();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> TypeTable {
        TypeTable::from_json_str(
            r#"{
              "format": 1,
              "root": "Verse.Def",
              "assemblies": ["RS_Core"],
              "types": {
                "RS_Plain": {"base": "Verse.Def"},
                "Verse.Def": {"base": "Verse.Editable"},
                "Verse.RS_ThingDef": {"base": "Verse.Def"},
                "RS_Mod.RS_AmmoDef": {"base": "Verse.RS_ThingDef"},
                "RS_Other.RS_Dup": {"base": "Verse.Def"},
                "RS_Third.RS_Dup": {"base": "Verse.Def"},
                "Verse.RS_Outer/RS_Inner": {"base": "Verse.Def"}
              },
              "short_name_collisions": {}
            }"#,
        )
        .unwrap()
    }

    #[test]
    fn short_names_work_only_for_ignored_namespaces_and_the_global_one() {
        let t = table();
        assert_eq!(t.lookup("RS_Plain"), Some("RS_Plain"));
        assert_eq!(t.lookup("RS_ThingDef"), Some("Verse.RS_ThingDef"));
        assert_eq!(t.lookup("RS_AmmoDef"), None);
    }

    #[test]
    fn full_names_are_case_insensitive() {
        let t = table();
        assert_eq!(t.lookup("RS_Mod.RS_AmmoDef"), Some("RS_Mod.RS_AmmoDef"));
        assert_eq!(t.lookup("rs_mod.rs_ammodef"), Some("RS_Mod.RS_AmmoDef"));
        assert_eq!(t.lookup("rs_thingdef"), Some("Verse.RS_ThingDef"));
        assert_eq!(t.lookup("nothing"), None);
    }

    #[test]
    fn nested_types_use_the_part_after_the_slash_as_short_name() {
        let t = table();
        assert_eq!(t.lookup("RS_Inner"), Some("Verse.RS_Outer/RS_Inner"));
    }

    #[test]
    fn reflection_spelling_of_nested_types_is_accepted() {
        let t = TypeTable::new(vec![(
            "Verse.RS_Outer+RS_Inner".into(),
            TypeInfo::with_base("Verse.Def"),
        )])
        .unwrap();
        assert_eq!(t.lookup("RS_Inner"), Some("Verse.RS_Outer+RS_Inner"));
        assert_eq!(
            t.lookup("verse.rs_outer+rs_inner"),
            Some("Verse.RS_Outer+RS_Inner")
        );
    }

    #[test]
    fn ancestors_end_at_the_root() {
        let t = table();
        assert_eq!(
            t.ancestors("RS_Mod.RS_AmmoDef"),
            vec!["RS_Mod.RS_AmmoDef", "Verse.RS_ThingDef", "Verse.Def"]
        );
        assert!(t.is_a("RS_Mod.RS_AmmoDef", "Verse.RS_ThingDef"));
        assert!(!t.is_a("RS_Plain", "Verse.RS_ThingDef"));
        assert!(t.ancestors("Missing").is_empty());
    }

    #[test]
    fn collisions_choose_the_short_name_target_even_outside_ignored_namespaces() {
        let t = table();
        // without the collision entry a short name outside the ignored namespaces is unavailable
        assert_eq!(t.lookup("RS_Dup"), None);
        let mut v = t.to_json_value();
        v["short_name_collisions"] =
            serde_json::json!({"RS_Dup": ["RS_Other.RS_Dup", "RS_Third.RS_Dup"]});
        let t = TypeTable::from_json_value(v).unwrap();
        assert_eq!(t.lookup("RS_Dup"), Some("RS_Third.RS_Dup"));
        assert_eq!(t.lookup("rs_other.rs_dup"), Some("RS_Other.RS_Dup"));
    }

    #[test]
    fn json_round_trip_keeps_the_order() {
        let t = table();
        let again = TypeTable::from_json_value(t.to_json_value()).unwrap();
        let a: Vec<&str> = t.iter().map(|(k, _)| k).collect();
        let b: Vec<&str> = again.iter().map(|(k, _)| k).collect();
        assert_eq!(a, b);
        assert_eq!(again.root(), "Verse.Def");
    }

    #[test]
    fn duplicate_types_and_bad_json_are_errors() {
        let err = TypeTable::new(vec![
            ("A".into(), TypeInfo::default()),
            ("A".into(), TypeInfo::default()),
        ]);
        assert!(matches!(err, Err(DefsError::InvalidTypeTable { .. })));
        let err = TypeTable::from_json_str("{\"types\": 5}").unwrap_err();
        assert_eq!(err.code(), "defs.type-table-invalid");
    }

    #[test]
    fn last_short_name_wins() {
        let t = TypeTable::new(vec![
            ("Verse.RS_Same".into(), TypeInfo::with_base("Verse.Def")),
            ("RimWorld.RS_Same".into(), TypeInfo::with_base("Verse.Def")),
        ])
        .unwrap();
        assert_eq!(t.lookup("RS_Same"), Some("RimWorld.RS_Same"));
    }
}
