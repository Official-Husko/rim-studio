//! Lint rules for the generated ammunition definitions: CEP050 to CEP057.
//!
//! The patch lint of [`crate::ce::lint`] reads `Patch` files; the definitions of a custom caliber are `Defs`
//! files, so their rules live here. [`lint_defs`] reads the parsed `Defs` roots (the files of the plan, or
//! hand written ones) and answers with diagnostics that carry the rule id (`ruleId`), the file (`path`) and
//! the def (`def`). A parent counts as known when the file defines it, when an installed ammunition def
//! names it as its parent, or when the caller lists it in `known`.

use std::collections::BTreeSet;

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::Node;

use super::codes::{CEP050, CEP051, CEP052, CEP053, CEP054, CEP055, CEP056, CEP057};
use super::names::{AMMO_DEF, PROJECTILE_PROPS};
use crate::ce::reader::CeModel;
use crate::reader::access::{child_text, class_attr, text_of};

fn tag_rule(d: Diagnostic, rule: &str, path: Option<&str>, def: &str) -> Diagnostic {
    let d = d.with_arg("ruleId", rule).with_arg("def", def);
    match path {
        Some(p) => d.with_arg("path", p),
        None => d,
    }
}

fn pairs_of(set: &Node) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for e in set.child("ammoTypes").into_iter().flat_map(Node::elements) {
        if e.tag == "li" {
            if let (Some(a), Some(p)) = (child_text(e, "ammo"), child_text(e, "projectile")) {
                out.push((a, p));
            }
        } else if let Some(p) = text_of(e) {
            out.push((e.tag.clone(), p));
        }
    }
    out
}

/// Checks `Defs` roots. `files` are `(path, root)` pairs. `known` holds other def names that exist (the
/// project's own defs and parents).
#[must_use]
pub fn lint_defs(
    files: &[(Option<String>, Node)],
    model: &CeModel,
    known: &BTreeSet<String>,
) -> Vec<Diagnostic> {
    let mut defined: BTreeSet<String> = known.clone();
    let mut parents: BTreeSet<String> = BTreeSet::new();
    let lib = &model.ammo;
    defined.extend(lib.ammo.keys().cloned());
    defined.extend(lib.projectiles.keys().cloned());
    defined.extend(model.ammo_sets.iter().map(|s| s.def_name.clone()));
    parents.extend(lib.ammo.values().filter_map(|a| a.parent.clone()));
    parents.extend(lib.projectiles.values().filter_map(|a| a.parent.clone()));
    parents.extend(lib.recipes.values().filter_map(|a| a.parent.clone()));
    for (_, root) in files {
        for d in root.elements() {
            if let Some(n) = child_text(d, "defName") {
                defined.insert(n);
            }
            if let Some(n) = d.attr("Name") {
                parents.insert(n.to_owned());
            }
        }
    }
    let mut out = Vec::new();
    for (path, root) in files {
        let path = path.as_deref();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for d in root.elements() {
            let def = child_text(d, "defName").unwrap_or_default();
            if !def.is_empty() && !seen.insert(def.clone()) {
                out.push(tag_rule(
                    CEP057.diagnostic("", &[("def", &def)]),
                    "CEP057",
                    path,
                    &def,
                ));
            }
            if let Some(p) = d.attr("ParentName")
                && !parents.contains(p)
                && !defined.contains(p)
            {
                out.push(tag_rule(
                    CEP050.diagnostic("", &[("def", &def), ("parent", p)]),
                    "CEP050",
                    path,
                    &def,
                ));
            }
            match d.tag.as_str() {
                tag if tag == model.classes.ammo_set_def => {
                    for (ammo, proj) in pairs_of(d) {
                        for missing in [&ammo, &proj] {
                            if !defined.contains(missing) {
                                out.push(tag_rule(
                                    CEP051.diagnostic(
                                        "",
                                        &[
                                            ("def", &def),
                                            ("ammo", &ammo),
                                            ("projectile", &proj),
                                            ("missing", missing),
                                        ],
                                    ),
                                    "CEP051",
                                    path,
                                    &def,
                                ));
                            }
                        }
                    }
                }
                "ThingDef" => thing_rules(d, &def, path, &defined, &mut out),
                "RecipeDef" => recipe_rules(d, &def, path, &defined, &mut out),
                _ => {}
            }
        }
    }
    out
}

fn thing_rules(
    d: &Node,
    def: &str,
    path: Option<&str>,
    defined: &BTreeSet<String>,
    out: &mut Vec<Diagnostic>,
) {
    if d.child("ammoClass").is_some() && class_attr(d) != Some(AMMO_DEF) {
        out.push(tag_rule(
            CEP054.diagnostic("", &[("def", def), ("what", "an ammo item")]),
            "CEP054",
            path,
            def,
        ));
    }
    for tag in ["cookOffProjectile", "detonateProjectile"] {
        if let Some(p) = child_text(d, tag)
            && !defined.contains(&p)
        {
            out.push(tag_rule(
                CEP052.diagnostic("", &[("def", def), ("projectile", &p)]),
                "CEP052",
                path,
                def,
            ));
        }
    }
    let Some(props) = d.child("projectile") else {
        return;
    };
    if class_attr(props) != Some(PROJECTILE_PROPS) {
        out.push(tag_rule(
            CEP055.diagnostic("", &[("def", def)]),
            "CEP055",
            path,
            def,
        ));
        return;
    }
    if props.child("explosionRadius").is_none() {
        for field in [
            "damageAmountBase",
            "armorPenetrationSharp",
            "armorPenetrationBlunt",
        ] {
            if props.child(field).is_none() {
                out.push(tag_rule(
                    CEP056.diagnostic("", &[("def", def), ("what", field)]),
                    "CEP056",
                    path,
                    def,
                ));
            }
        }
    }
}

fn recipe_rules(
    d: &Node,
    def: &str,
    path: Option<&str>,
    defined: &BTreeSet<String>,
    out: &mut Vec<Diagnostic>,
) {
    let products: Vec<String> = d
        .child("products")
        .into_iter()
        .flat_map(Node::elements)
        .map(|e| e.tag.clone())
        .collect();
    if products.is_empty() {
        out.push(tag_rule(
            CEP053.diagnostic("", &[("def", def), ("problem", "makes nothing")]),
            "CEP053",
            path,
            def,
        ));
    }
    for p in products.iter().filter(|p| !defined.contains(*p)) {
        out.push(tag_rule(
            CEP053.diagnostic(
                "",
                &[
                    ("def", def),
                    ("problem", &format!("makes {p}, which is not defined")),
                ],
            ),
            "CEP053",
            path,
            def,
        ));
    }
    if d.child("ingredients")
        .is_none_or(|i| i.elements().next().is_none())
    {
        out.push(tag_rule(
            CEP053.diagnostic("", &[("def", def), ("problem", "has no ingredient")]),
            "CEP053",
            path,
            def,
        ));
    }
}
