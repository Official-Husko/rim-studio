//! The dry load: the generated definitions loaded through the def engine with the Combat Extended type table,
//! to confirm that they resolve as the types the game reads them as.
//!
//! The parents the definitions name are not part of the generated file (they belong to the installed Combat
//! Extended), so the dry load adds an empty abstract def of the right kind for each of them. What is
//! checked is what the file itself decides: the element names and classes are known types, every def gets a
//! database entry of the expected type (an ammo item as the ammo def type, the ammo set as the ammo set def
//! type, a projectile as a thing def, a recipe as a recipe def), no def name repeats, and the engine reports
//! no error.

use std::collections::BTreeSet;
use std::sync::Arc;

use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::tree::Node;
use rimstudio_defs::{DefFile, FileContent, LoadInput, ModEntry, TypeInfo, TypeTable, load};

use super::codes::DRY_LOAD_FAILED;
use super::names::{AMMO_DEF, type_entries};
use crate::ce::reader::CeModel;

/// The outcome of a dry load.
#[derive(Debug, Clone, PartialEq)]
pub struct DryLoad {
    /// One error per def that does not load as expected, and one per error of the engine.
    pub diagnostics: Vec<Diagnostic>,
    /// The defs that loaded, as `(database type, def name)`.
    pub loaded: Vec<(String, String)>,
}

impl DryLoad {
    /// True when nothing failed.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        !self
            .diagnostics
            .iter()
            .any(|d| d.severity == Severity::Error)
    }
}

fn table(model: &CeModel) -> Option<Arc<TypeTable>> {
    let mut list: Vec<(String, TypeInfo)> = vec![
        (
            "Verse.Def".to_owned(),
            TypeInfo::with_base("Verse.Editable"),
        ),
        (
            "Verse.ThingDef".to_owned(),
            TypeInfo::with_base("Verse.Def"),
        ),
        (
            "Verse.RecipeDef".to_owned(),
            TypeInfo::with_base("Verse.Def"),
        ),
        (
            "Verse.ThingCategoryDef".to_owned(),
            TypeInfo::with_base("Verse.Def"),
        ),
    ];
    let mut extra = type_entries();
    extra.push((model.classes.ammo_set_def.clone(), "Verse.Def".to_owned()));
    for (name, base) in extra {
        list.push((name, TypeInfo::with_base(base)));
    }
    TypeTable::new(list).ok().map(Arc::new)
}

fn expected_type(node: &Node, model: &CeModel) -> String {
    match (node.tag.as_str(), node.attr("Class")) {
        ("ThingDef", Some(class)) => class.to_owned(),
        (tag, _) if tag == model.classes.ammo_set_def => model.classes.ammo_set_def.clone(),
        (tag, _) => tag.to_owned(),
    }
}

/// Loads the generated defs (the children of the `Defs` root of the file) as one mod.
#[must_use]
pub fn dry_load(defs: &[Node], model: &CeModel) -> DryLoad {
    let mut out = DryLoad {
        diagnostics: Vec::new(),
        loaded: Vec::new(),
    };
    let Some(types) = table(model) else {
        out.diagnostics.push(DRY_LOAD_FAILED.diagnostic(
            "",
            &[
                ("def", "the file"),
                ("expected", "defs"),
                (
                    "reason",
                    "the type table of the dry load could not be built",
                ),
            ],
        ));
        return out;
    };
    let mut root = Node::new("Defs");
    let mut stubs: BTreeSet<(String, String, Option<String>)> = BTreeSet::new();
    for d in defs {
        if let Some(parent) = d.attr("ParentName") {
            stubs.insert((
                parent.to_owned(),
                d.tag.clone(),
                d.attr("Class").map(str::to_owned),
            ));
        }
    }
    let mut seen_stub: BTreeSet<String> = BTreeSet::new();
    for (name, tag, class) in stubs {
        if !seen_stub.insert(name.clone()) {
            continue;
        }
        let mut stub = Node::new(tag);
        stub.set_attr("Name", name);
        stub.set_attr("Abstract", "True");
        if let Some(c) = class {
            stub.set_attr("Class", c);
        }
        root.push_child(stub);
    }
    for d in defs {
        root.push_child(d.clone());
    }
    let mut input = LoadInput::new(
        vec![ModEntry::new(ModIdx(0), "rs.dryload", "dry load")],
        types,
    );
    input.def_files.push(DefFile {
        mod_idx: ModIdx(0),
        file: FileId(0),
        rel_path: "Defs/Ammo.xml".to_owned(),
        content: FileContent::parsed(root),
    });
    let loaded = load(input);
    let mut names: BTreeSet<String> = BTreeSet::new();
    for d in defs {
        let name = d.child_text("defName").unwrap_or_default().to_owned();
        let expected = expected_type(d, model);
        if !names.insert(name.clone()) {
            out.diagnostics.push(DRY_LOAD_FAILED.diagnostic(
                "",
                &[
                    ("def", &name),
                    ("expected", &expected),
                    ("reason", "the def name is used twice"),
                ],
            ));
            continue;
        }
        let db = match d.tag.as_str() {
            "ThingDef" if d.attr("Class") == Some(AMMO_DEF) => AMMO_DEF.to_owned(),
            tag if tag == model.classes.ammo_set_def => model.classes.ammo_set_def.clone(),
            "ThingDef" => "ThingDef".to_owned(),
            tag => tag.to_owned(),
        };
        match loaded.get(&db, &name) {
            Some(rec) if rec.type_name.ends_with(expected_short(&expected)) => {
                out.loaded.push((db, name));
            }
            Some(rec) => out.diagnostics.push(DRY_LOAD_FAILED.diagnostic("", &[("def", &name), ("expected", &expected), ("reason", &format!("it loaded as {}", rec.type_name))])),
            None => out.diagnostics.push(DRY_LOAD_FAILED.diagnostic("", &[("def", &name), ("expected", &expected), ("reason", "no def of that name was created (an unknown type, an unresolved parent or a repeated name)")])),
        }
    }
    for d in loaded
        .diagnostics
        .samples
        .iter()
        .filter(|d| d.severity == Severity::Error)
    {
        out.diagnostics.push(DRY_LOAD_FAILED.diagnostic(
            "",
            &[
                ("def", d.code.as_str()),
                ("expected", "a clean load"),
                ("reason", &d.message),
            ],
        ));
    }
    out
}

fn expected_short(expected: &str) -> &str {
    expected.rsplit('.').next().unwrap_or(expected)
}
