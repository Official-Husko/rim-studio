//! A fictional Combat Extended ammunition install for the custom ammo tests. Every name starts with `RS_`
//! and every number is invented by a simple formula; nothing here comes from an install. The defs go through
//! the real def engine and the real Combat Extended reader, so the library the tests use is what the reader
//! makes of defs, not a hand built structure.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

use std::sync::Arc;

use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::tree::{Node, NodeBuilder};
use rimstudio_defs::{DefFile, FileContent, LoadInput, ModEntry, TypeInfo, TypeTable, load};
use rimstudio_design::ce::reader::{
    CeClassNames, CeModel, CeNames, CeReadOptions, read_conversions_with, with_ce_types,
};

/// Calibers of the fictional install: `(set name, caliber label, family category, base damage, base speed)`.
pub(crate) const CALIBERS: [(&str, &str, &str, f64, f64); 5] = [
    ("Pistol9", "9mm test", "RS_AmmoPistols", 10.0, 90.0),
    ("Pistol10", "10mm test", "RS_AmmoPistols", 12.0, 95.0),
    ("Rifle5", "5.5mm test", "RS_AmmoRifles", 14.0, 150.0),
    ("Rifle7", "7mm test", "RS_AmmoRifles", 18.0, 160.0),
    ("Rifle8", "8mm test", "RS_AmmoRifles", 22.0, 165.0),
];

/// The three bullet classes: `(def name, label, short label, damage factor, penetration factor)`.
pub(crate) const CLASSES: [(&str, &str, &str, f64, f64); 3] = [
    ("RS_FMJ", "full metal jacket", "FMJ", 1.0, 0.5),
    ("RS_AP", "armor piercing", "AP", 0.7, 1.0),
    ("RS_HP", "hollow point", "HP", 1.3, 0.2),
];

fn leaf(tag: &str, text: impl Into<String>) -> Node {
    Node::with_text(tag, text)
}

fn class_def(def: &str, label: &str, short: &str) -> Node {
    NodeBuilder::new("CombatExtended.AmmoCategoryDef")
        .text_elem("defName", def)
        .text_elem("label", label)
        .text_elem("labelShort", short)
        .text_elem("description", format!("{label} rounds"))
        .build()
}

fn category(def: &str, label: &str, parent: Option<&str>) -> Node {
    NodeBuilder::new("ThingCategoryDef")
        .text_elem("defName", def)
        .text_elem("label", label)
        .text_elem_opt("parent", parent)
        .text_elem("iconPath", "RS/Icons/Category")
        .build()
}

fn ammo_base() -> Vec<Node> {
    vec![
        NodeBuilder::new("ThingDef")
            .attr("Class", "CombatExtended.AmmoDef")
            .attr("Name", "RS_AmmoBase")
            .attr("Abstract", "True")
            .text_elem("thingClass", "CombatExtended.AmmoThing")
            .text_elem("category", "Item")
            .text_elem("techLevel", "Industrial")
            .elem("tradeTags", |t| t.li("RS_Injector").li("RS_Ammo"))
            .text_elem("stackLimit", "5000")
            .build(),
        NodeBuilder::new("ThingDef")
            .attr("Name", "RS_BulletBase")
            .attr("Abstract", "True")
            .text_elem("thingClass", "CombatExtended.BulletCE")
            .text_elem("category", "Projectile")
            .elem("graphicData", |g| {
                g.text_elem("texPath", "RS/Projectile/Bullet")
                    .text_elem("graphicClass", "Graphic_Single")
            })
            .elem("projectile", |p| {
                p.attr("Class", "CombatExtended.ProjectilePropertiesCE")
                    .text_elem("damageDef", "RS_Bullet")
                    .text_elem("dropsCasings", "true")
                    .text_elem("casingMoteDefname", "RS_FleckCasing")
            })
            .build(),
        NodeBuilder::new("RecipeDef")
            .attr("Name", "RS_AmmoRecipeBase")
            .attr("Abstract", "True")
            .text_elem("workSkill", "Crafting")
            .text_elem("workAmount", "1000")
            .elem("recipeUsers", |u| u.li("RS_Bench"))
            .build(),
    ]
}

/// Ammo item, projectile and recipe of one type of one caliber.
fn type_defs(
    set: &str,
    caliber_label: &str,
    family: &str,
    class: usize,
    base_damage: f64,
    base_speed: f64,
    n: usize,
) -> Vec<Node> {
    let (class_def, _, short, dmg_factor, ap_factor) = CLASSES[class];
    let ammo = format!("RS_Ammo_{set}_{short}");
    let bullet = format!("RS_Bullet_{set}_{short}");
    let damage = (base_damage * dmg_factor).round();
    let ap = (base_damage * ap_factor).round();
    vec![
        NodeBuilder::new("ThingDef")
            .attr("Class", "CombatExtended.AmmoDef")
            .attr("ParentName", "RS_AmmoBase")
            .text_elem("defName", &ammo)
            .text_elem("label", format!("{caliber_label} ({short})"))
            .text_elem("description", format!("Test cartridge {caliber_label}."))
            .elem("statBases", |s| {
                s.text_elem("Mass", format!("{}", 0.01 + 0.002 * n as f64))
                    .text_elem("Bulk", format!("{}", 0.02 + 0.004 * n as f64))
            })
            .elem("thingCategories", |c| c.li(format!("RS_Ammo{set}")))
            .elem("graphicData", |g| {
                g.text_elem("texPath", format!("RS/Ammo/{set}/{short}"))
                    .text_elem("graphicClass", "Graphic_StackCount")
            })
            .text_elem("ammoClass", class_def)
            .text_elem("cookOffProjectile", &bullet)
            .build(),
        NodeBuilder::new("ThingDef")
            .attr("ParentName", "RS_BulletBase")
            .text_elem("defName", &bullet)
            .text_elem("label", format!("{caliber_label} bullet ({short})"))
            .elem("projectile", |p| {
                p.attr("Class", "CombatExtended.ProjectilePropertiesCE")
                    .text_elem("damageAmountBase", damage.to_string())
                    .text_elem("armorPenetrationSharp", ap.to_string())
                    .text_elem("armorPenetrationBlunt", (base_damage * 1.5).to_string())
                    .text_elem("speed", base_speed.to_string())
            })
            .build(),
        NodeBuilder::new("RecipeDef")
            .attr("ParentName", "RS_AmmoRecipeBase")
            .text_elem("defName", format!("RS_MakeAmmo_{set}_{short}"))
            .text_elem("label", format!("make {caliber_label} ({short}) x500"))
            .text_elem(
                "description",
                format!("Craft 500 {caliber_label} ({short})."),
            )
            .text_elem("jobString", format!("Making {caliber_label} ({short})."))
            .elem("ingredients", |i| {
                i.elem("li", |li| {
                    li.elem("filter", |f| f.elem("thingDefs", |t| t.li("RS_Steel")))
                        .text_elem("count", (10 + n).to_string())
                })
            })
            .elem("fixedIngredientFilter", |f| {
                f.elem("thingDefs", |t| t.li("RS_Steel"))
            })
            .elem("products", |p| p.text_elem(&ammo, "500"))
            .text_elem("workAmount", (1500 + 100 * class + 10 * n).to_string())
            .text_elem("researchPrerequisite", "RS_Research")
            .build(),
        {
            let _ = family;
            leaf("Unused", "")
        },
    ]
    .into_iter()
    .filter(|n| n.tag != "Unused")
    .collect()
}

/// The defs of the fictional install.
pub(crate) fn defs() -> Vec<Node> {
    let mut out: Vec<Node> = Vec::new();
    for (def, label, short, _, _) in CLASSES {
        out.push(class_def(def, label, short));
    }
    out.push(class_def("RS_HE", "high explosive", "HE"));
    out.push(category("RS_Ammo", "ammunition", None));
    out.push(category("RS_AmmoPistols", "pistol", Some("RS_Ammo")));
    out.push(category("RS_AmmoRifles", "rifle", Some("RS_Ammo")));
    out.extend(ammo_base());
    for (n, (set, label, family, damage, speed)) in CALIBERS.iter().enumerate() {
        out.push(category(&format!("RS_Ammo{set}"), label, Some(family)));
        let mut pairs = Node::new("ammoTypes");
        for (c, (_, _, short, _, _)) in CLASSES.iter().enumerate() {
            let _ = c;
            pairs.push_child(leaf(
                &format!("RS_Ammo_{set}_{short}"),
                format!("RS_Bullet_{set}_{short}"),
            ));
        }
        let mut set_def = NodeBuilder::new("CombatExtended.AmmoSetDef")
            .text_elem("defName", format!("RS_AmmoSet_{set}"))
            .text_elem("label", *label)
            .child(pairs);
        if n == 4 {
            set_def = set_def.text_elem("similarTo", "RS_AmmoSet_Pistol9");
        }
        out.push(set_def.build());
        for c in 0..CLASSES.len() {
            out.extend(type_defs(set, label, family, c, *damage, *speed, n));
        }
    }
    // an explosive caliber: one grenade type with secondary damage and fragments
    out.push(category("RS_AmmoGrenade", "grenade test", Some("RS_Ammo")));
    out.push(
        NodeBuilder::new("CombatExtended.AmmoSetDef")
            .text_elem("defName", "RS_AmmoSet_Grenade")
            .text_elem("label", "grenade test")
            .elem("ammoTypes", |p| {
                p.text_elem("RS_Ammo_Grenade_HE", "RS_Bullet_Grenade_HE")
            })
            .build(),
    );
    out.push(
        NodeBuilder::new("ThingDef")
            .attr("Class", "CombatExtended.AmmoDef")
            .attr("ParentName", "RS_AmmoBase")
            .text_elem("defName", "RS_Ammo_Grenade_HE")
            .text_elem("label", "grenade test (HE)")
            .elem("statBases", |s| {
                s.text_elem("Mass", "0.2").text_elem("Bulk", "0.4")
            })
            .elem("thingCategories", |c| c.li("RS_AmmoGrenade"))
            .elem("graphicData", |g| {
                g.text_elem("texPath", "RS/Ammo/Grenade/HE")
                    .text_elem("graphicClass", "Graphic_StackCount")
            })
            .text_elem("ammoClass", "RS_HE")
            .text_elem("detonateProjectile", "RS_Bullet_Grenade_HE")
            .text_elem("generateAllowChance", "0.5")
            .build(),
    );
    out.push(
        NodeBuilder::new("ThingDef")
            .attr("ParentName", "RS_BulletBase")
            .text_elem("defName", "RS_Bullet_Grenade_HE")
            .text_elem("label", "grenade test (HE)")
            .text_elem("thingClass", "CombatExtended.ProjectileCE_Explosive")
            .elem("projectile", |p| {
                p.attr("Class", "CombatExtended.ProjectilePropertiesCE")
                    .text_elem("damageDef", "RS_Bomb")
                    .text_elem("damageAmountBase", "22")
                    .text_elem("explosionRadius", "1.5")
                    .text_elem("speed", "27")
                    .elem("secondaryDamage", |s| {
                        s.elem("li", |li| {
                            li.text_elem("def", "RS_BombSecondary")
                                .text_elem("amount", "4")
                        })
                    })
            })
            .elem("comps", |c| {
                c.elem("li", |li| {
                    li.attr("Class", "CombatExtended.CompProperties_Fragments")
                        .elem("fragments", |f| f.text_elem("RS_FragmentSmall", "20"))
                })
            })
            .build(),
    );
    out
}

/// The type table of the fictional install, with the Combat Extended types.
pub(crate) fn types() -> Arc<TypeTable> {
    let base = TypeTable::new(vec![
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
    ])
    .unwrap();
    Arc::new(with_ce_types(&base, &CeClassNames::default()).unwrap())
}

/// The model the reader makes of the fictional install, with a few converted guns that use its sets.
pub(crate) fn model() -> CeModel {
    let mut root = Node::new("Defs");
    for d in defs() {
        root.push_child(d);
    }
    let order = vec![ModEntry::new(ModIdx(0), "RS.CombatExt", "RS Combat Ext")];
    let mut input = LoadInput::new(order, types());
    input.def_files.push(DefFile {
        mod_idx: ModIdx(0),
        file: FileId(0),
        rel_path: "Defs/Ammo.xml".to_owned(),
        content: FileContent::parsed(root),
    });
    let out = load(input);
    let opts = CeReadOptions {
        order: Some(&out.order),
        ..CeReadOptions::default()
    };
    let mut model = read_conversions_with(&out.databases, &opts);
    model.names = Some(CeNames {
        package_id: "RS.CombatExt".into(),
        name: "RS Combat Ext".into(),
    });
    let base = super::common_ce::ce_model();
    model.weapon_tags = base.weapon_tags.clone();
    model.ai_class_tags = base.ai_class_tags.clone();
    model.melee = base.melee.clone();
    model.guns = base
        .guns
        .into_iter()
        .enumerate()
        .map(|(i, mut g)| {
            g.ammo_set = Some(
                if i < 4 {
                    "RS_AmmoSet_Pistol9"
                } else {
                    "RS_AmmoSet_Rifle7"
                }
                .into(),
            );
            g.default_projectile = Some(
                if i < 4 {
                    "RS_Bullet_Pistol9_FMJ"
                } else {
                    "RS_Bullet_Rifle7_FMJ"
                }
                .into(),
            );
            g
        })
        .collect();
    model
}

/// One difference between a real def and the def made from its rebuilt spec.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Diff {
    /// `missing` (the real def has it, the generated one lacks it), `different` or `extra`.
    pub(crate) kind: &'static str,
    /// The path of the element, list entries as `li`.
    pub(crate) path: String,
}

fn leaves(node: &Node, path: &str, out: &mut Vec<(String, String)>) {
    let kids: Vec<&Node> = node.elements().collect();
    if kids.is_empty() {
        let t = node.text_content().trim().to_owned();
        let v = t.parse::<f64>().map_or(t.clone(), |f| format!("{f}"));
        out.push((path.to_owned(), v.to_lowercase()));
        return;
    }
    for k in kids {
        let seg = if k.tag == "li" {
            match (k.attr("Class"), k.child_text("def")) {
                (Some(c), _) => format!("li[{c}]"),
                (None, Some(d)) => format!("li[{d}]"),
                _ => "li".to_owned(),
            }
        } else {
            k.tag.clone()
        };
        leaves(k, &format!("{path}/{seg}"), out);
    }
}

/// Compares two resolved defs leaf by leaf (a path that repeats is compared as a sorted list of values).
/// Paths in `ignore` (prefix match) are skipped.
pub(crate) fn compare(real: &Node, generated: &Node, ignore: &[&str]) -> Vec<Diff> {
    let (mut a, mut b) = (Vec::new(), Vec::new());
    leaves(real, "", &mut a);
    leaves(generated, "", &mut b);
    let group = |v: Vec<(String, String)>| {
        let mut m: std::collections::BTreeMap<String, Vec<String>> =
            std::collections::BTreeMap::new();
        for (p, x) in v {
            m.entry(p).or_default().push(x);
        }
        for list in m.values_mut() {
            list.sort();
        }
        m
    };
    let (a, b) = (group(a), group(b));
    let skip = |p: &str| ignore.iter().any(|i| p.starts_with(i));
    let mut out = Vec::new();
    for (p, v) in &a {
        if skip(p) {
            continue;
        }
        match b.get(p) {
            None => out.push(Diff {
                kind: "missing",
                path: p.clone(),
            }),
            Some(w) if w != v => out.push(Diff {
                kind: "different",
                path: p.clone(),
            }),
            Some(_) => {}
        }
    }
    for p in b.keys() {
        if !skip(p) && !a.contains_key(p) {
            out.push(Diff {
                kind: "extra",
                path: p.clone(),
            });
        }
    }
    out.sort();
    out
}

use std::collections::BTreeMap;

use rimstudio_defs::DefDatabases;
use rimstudio_design::ce::ammo::{self, DerivedNames, GeneratedAmmo};
use rimstudio_design::model::{CustomAmmoSpec, ValueSource};
use rimstudio_design::plan::ProjectLayout;

/// A real set rebuilt as a custom spec and its generated definitions.
pub(crate) struct Rebuilt {
    pub(crate) set: String,
    pub(crate) spec: CustomAmmoSpec,
    pub(crate) made: GeneratedAmmo,
}

/// Rebuilds every ammo set of the model as a custom spec (typed values) and generates its definitions under
/// `prefix`.
pub(crate) fn rebuild_all(model: &CeModel, prefix: &str) -> Vec<Rebuilt> {
    let layout = ProjectLayout::default();
    model
        .ammo_sets
        .iter()
        .filter_map(|s| {
            let spec = ammo::set_from_existing(model, &s.def_name, ValueSource::Typed)?;
            let made = ammo::generate(&spec, prefix, model, &layout);
            Some(Rebuilt {
                set: s.def_name.clone(),
                spec,
                made,
            })
        })
        .collect()
}

/// The counts of a comparison: `"<kind> <missing|different|extra> <path>"` to the number of occurrences.
#[derive(Default)]
pub(crate) struct Report {
    pub(crate) sets: usize,
    pub(crate) types: usize,
    pub(crate) unresolved: usize,
    pub(crate) diffs: BTreeMap<String, usize>,
    /// The first pair `real=generated` seen for each difference.
    pub(crate) examples: BTreeMap<String, String>,
}

impl Report {
    fn add(&mut self, kind: &str, diffs: Vec<Diff>, pair: &str) {
        for d in diffs {
            let key = format!("{kind} {} {}", d.kind, d.path);
            self.examples
                .entry(key.clone())
                .or_insert_with(|| pair.to_owned());
            *self.diffs.entry(key).or_insert(0) += 1;
        }
    }

    /// The number of differences of a kind (`missing`, `different`, `extra`).
    pub(crate) fn count(&self, what: &str) -> usize {
        self.diffs
            .iter()
            .filter(|(k, _)| k.split(' ').nth(1) == Some(what))
            .map(|(_, n)| n)
            .sum()
    }
}

/// Compares every real set, ammo item, projectile and recipe with the generated one. `dbs` holds both loads.
pub(crate) fn compare_all(
    model: &CeModel,
    prefix: &str,
    rebuilt: &[Rebuilt],
    dbs: &DefDatabases,
) -> Report {
    let mut report = Report::default();
    let set_type = model.classes.ammo_set_def.clone();
    for r in rebuilt {
        report.sets += 1;
        let names = DerivedNames::new(&r.spec, prefix);
        let Some(real_set) = dbs.get(&set_type, &r.set) else {
            continue;
        };
        let Some(gen_set) = dbs.get(&set_type, &names.set()) else {
            report.unresolved += 1;
            continue;
        };
        report.add(
            "set",
            compare(&real_set.node, &gen_set.node, &["/defName", "/ammoTypes"]),
            &format!("{}={}", r.set, names.set()),
        );
        let real_pairs = real_set
            .node
            .child("ammoTypes")
            .map_or(0, |p| p.elements().count());
        let gen_pairs = gen_set
            .node
            .child("ammoTypes")
            .map_or(0, |p| p.elements().count());
        if real_pairs != gen_pairs {
            *report
                .diffs
                .entry("set different /ammoTypes".into())
                .or_insert(0) += 1;
        }
        let pairs: Vec<(String, String)> = real_set
            .node
            .child("ammoTypes")
            .map(|p| {
                p.elements()
                    .map(|e| (e.tag.clone(), e.text_content().trim().to_owned()))
                    .collect()
            })
            .unwrap_or_default();
        for (t, (ammo, projectile)) in r.spec.types.iter().zip(pairs.iter()) {
            report.types += 1;
            let key = ammo::type_key(t);
            for (kind, db, real, generated, ignore) in [
                (
                    "ammo",
                    "ThingDef",
                    ammo.clone(),
                    names.ammo(&key),
                    vec!["/defName", "/cookOffProjectile", "/detonateProjectile"],
                ),
                (
                    "projectile",
                    "ThingDef",
                    projectile.clone(),
                    names.projectile(&key),
                    vec!["/defName"],
                ),
                (
                    "recipe",
                    "RecipeDef",
                    model
                        .ammo
                        .recipes
                        .get(ammo)
                        .map(|x| x.def_name.clone())
                        .unwrap_or_default(),
                    names.recipe(&key),
                    vec!["/defName", "/products"],
                ),
            ] {
                let (Some(a), Some(b)) = (dbs.get(db, &real), dbs.get(db, &generated)) else {
                    if kind != "recipe" || !real.is_empty() {
                        report.unresolved += 1;
                    }
                    continue;
                };
                report.add(
                    kind,
                    compare(&a.node, &b.node, &ignore),
                    &format!("{real}={generated}"),
                );
            }
        }
    }
    report
}

/// The real defs and the generated defs of a rebuilt set as one load: the generated file is a second mod
/// after the original defs.
pub(crate) fn load_with(
    defs: Vec<Node>,
    rebuilt: &[Rebuilt],
    table: &Arc<TypeTable>,
) -> rimstudio_defs::LoadOutput {
    let mut a = Node::new("Defs");
    for d in defs {
        a.push_child(d);
    }
    let mut b = Node::new("Defs");
    for r in rebuilt {
        for d in &r.made.defs {
            b.push_child(d.clone());
        }
    }
    let order = vec![
        ModEntry::new(ModIdx(0), "RS.CombatExt", "RS Combat Ext"),
        ModEntry::new(ModIdx(1), "RS.Generated", "RS Generated"),
    ];
    let mut input = LoadInput::new(order, table.clone());
    for (i, root) in [a, b].into_iter().enumerate() {
        input.def_files.push(DefFile {
            mod_idx: ModIdx(i as u32),
            file: FileId(i as u32),
            rel_path: format!("Defs/Part{i}.xml"),
            content: FileContent::parsed(root),
        });
    }
    load(input)
}
