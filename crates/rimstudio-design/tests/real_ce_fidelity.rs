//! Round trip fidelity of the Combat Extended patch generator against the user's own Combat Extended.
//!
//! For every weapon that Combat Extended converts and that has a vanilla twin:
//!
//! 1. the real conversion is read as the Combat Extended block of a design of the vanilla twin (typed
//!    values, so only the structure of the output is tested);
//! 2. the generator produces its patch ([`gun_patch`], [`melee_patch`]);
//! 3. the patch is applied to the real vanilla defs by the def engine (one load with the generated patch as
//!    an extra mod), and once more twice over for the idempotence check;
//! 4. the resolved result is compared with the resolved def of the Combat Extended load. The comparison works
//!    on the change set against the vanilla def: every element and list entry that Combat Extended's own
//!    conversion adds or removes must be added or removed the same way, and every extra element of the
//!    generated patch is reported.
//!
//! Differences are grouped by element path with counts. The output is for a human to read; the only
//! assertions are the structural targets (no missing element, no element left that Combat Extended removes),
//! which are checked when `RIMSTUDIO_FIDELITY_STRICT` is set.
//!
//! Needs `RIMSTUDIO_GAME_DIR` and `RIMSTUDIO_CE_DIR`. Run with `cargo test -p rimstudio-design --release
//! --test real_ce_fidelity -- --ignored --nocapture --test-threads=1`. Nothing is written.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common_real;

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use common_real::{game_dir, official, prepare, type_table, version};
use rimstudio_core::ids::{FileId, ModIdx};
use rimstudio_core::tree::Node;
use rimstudio_defs::{DefDatabases, FileContent, ModEntry, PatchFile, load};
use rimstudio_design::ce::patchgen::{
    Container, ConversionSource, ConvertStatus, gun_patch, melee_patch, scan,
};
use rimstudio_design::ce::reader::{
    CeClassNames, CeModel, CeReadOptions, ce_block_from_def, custom_registry,
    read_conversions_with, with_ce_types,
};
use rimstudio_design::model::{CeToolPenetration, ItemKind, ValueSource};
use rimstudio_design::reader::{OwnSource, ReaderOptions, spec_from_def, spec_from_def_own};

const THING: &str = "ThingDef";

type Pairs = BTreeMap<(String, String), usize>;

/// The key segment of an element: its tag, with the class and the label for list entries.
fn segment(el: &Node) -> String {
    if el.tag != "li" {
        return el.tag.clone();
    }
    let mut s = "li".to_owned();
    if let Some(c) = el.attr("Class") {
        s.push_str(&format!("[@Class={c}]"));
    }
    if let Some(l) = el.child_text("label") {
        s.push_str(&format!("[label={}]", l.trim()));
    }
    s
}

/// A leaf value: numbers are compared as numbers.
fn value_of(n: &Node) -> String {
    let t = n.text_content().trim().to_owned();
    match t.parse::<f64>() {
        Ok(v) if v.is_finite() => format!("{v}"),
        _ => t.to_lowercase_if_bool(),
    }
}

trait BoolCase {
    fn to_lowercase_if_bool(self) -> String;
}

impl BoolCase for String {
    fn to_lowercase_if_bool(self) -> String {
        if self.eq_ignore_ascii_case("true") || self.eq_ignore_ascii_case("false") {
            self.to_lowercase()
        } else {
            self
        }
    }
}

fn walk(n: &Node, path: &str, out: &mut Pairs) {
    let kids: Vec<&Node> = n.elements().collect();
    if kids.is_empty() {
        *out.entry((path.to_owned(), value_of(n))).or_default() += 1;
        return;
    }
    // Tools are matched by position: Combat Extended relabels some tools, and the class of a tool entry is
    // part of what is compared.
    let positional = n.tag == "tools";
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    for k in &kids {
        *totals.entry(segment(k)).or_default() += 1;
    }
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    for (pos, k) in kids.into_iter().enumerate() {
        let key = if positional {
            format!("li@{pos}")
        } else {
            segment(k)
        };
        let idx = seen.entry(key.clone()).or_default();
        let p = if !positional && totals[&key] > 1 && k.elements().next().is_some() {
            format!("{path}/{key}#{idx}")
        } else {
            format!("{path}/{key}")
        };
        *idx += 1;
        if positional && let Some(c) = k.attr("Class") {
            *out.entry((format!("{p}/@Class"), c.to_owned()))
                .or_default() += 1;
        }
        walk(k, &p, out);
    }
}

fn flatten(def: &Node) -> Pairs {
    let mut out = Pairs::new();
    walk(def, "", &mut out);
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Kind {
    Missing,
    NotRemoved,
    Value,
    Extra,
    RemovedExtra,
}

struct Finding {
    weapon: String,
    kind: Kind,
    path: String,
    real: String,
    ours: String,
}

fn by_path(p: &Pairs) -> BTreeMap<String, Vec<String>> {
    let mut m: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for ((path, v), n) in p {
        for _ in 0..*n {
            m.entry(path.clone()).or_default().push(v.clone());
        }
    }
    m
}

/// Removes the values of `other` from `values` one by one and returns what is left.
fn take_common(values: &mut Vec<String>, other: &mut Vec<String>) {
    let mut keep = Vec::new();
    for x in std::mem::take(values) {
        if let Some(i) = other.iter().position(|y| *y == x) {
            other.remove(i);
        } else {
            keep.push(x);
        }
    }
    *values = keep;
}

/// Compares the real conversion with ours, both against the vanilla def, path by path.
///
/// - `Missing`: the real def has an entry that ours lacks (and the vanilla def did not have it either).
/// - `NotRemoved`: the real conversion removed an entry of the vanilla def that ours keeps.
/// - `RemovedExtra`: ours removed an entry of the vanilla def that the real def keeps.
/// - `Extra`: ours has an entry that neither the real def nor the vanilla def has.
/// - `Value`: both have the element with a different value.
fn compare(weapon: &str, vanilla: &Node, real: &Node, ours: &Node) -> Vec<Finding> {
    let v = by_path(&flatten(vanilla));
    let r = by_path(&flatten(real));
    let o = by_path(&flatten(ours));
    let mut out = Vec::new();
    let paths: BTreeSet<&String> = v.keys().chain(r.keys()).chain(o.keys()).collect();
    let mut push = |kind, path: &str, real: String, ours: String| {
        out.push(Finding {
            weapon: weapon.to_owned(),
            kind,
            path: path.to_owned(),
            real,
            ours,
        });
    };
    for path in paths {
        let vv = v.get(path).cloned().unwrap_or_default();
        let mut rv = r.get(path).cloned().unwrap_or_default();
        let mut ov = o.get(path).cloned().unwrap_or_default();
        take_common(&mut rv, &mut ov);
        if rv.is_empty() && ov.is_empty() {
            continue;
        }
        let set_like = path.ends_with("/li");
        if !set_like {
            while !rv.is_empty() && !ov.is_empty() {
                push(Kind::Value, path, rv.remove(0), ov.remove(0));
            }
        }
        for x in rv {
            // the real def has it, ours does not
            if vv.contains(&x) {
                push(Kind::RemovedExtra, path, x, String::new());
            } else {
                push(Kind::Missing, path, x, String::new());
            }
        }
        for x in ov {
            if vv.contains(&x) {
                push(Kind::NotRemoved, path, String::new(), x);
            } else {
                push(Kind::Extra, path, String::new(), x);
            }
        }
    }
    out
}

/// The path with the list entry labels and duplicate indexes replaced, for grouping.
fn group_key(path: &str) -> String {
    let mut out = String::new();
    let mut rest = path;
    while let Some(i) = rest.find("[label=") {
        out.push_str(&rest[..i]);
        out.push_str("[label=*]");
        match rest[i..].find(']') {
            Some(j) => rest = &rest[i + j + 1..],
            None => {
                rest = "";
            }
        }
    }
    out.push_str(rest);
    // drop the duplicate indexes
    let mut clean = String::new();
    let mut chars = out.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '#' {
            while chars.peek().is_some_and(char::is_ascii_digit) {
                chars.next();
            }
        } else {
            clean.push(c);
        }
    }
    clean
}

/// Why a missing or not removed path is not a defect of the generator, or `None` when it is unexplained.
/// The reasons are the classes of the research note `ce-structure-fidelity-0.1.0`.
fn explained(path: &str) -> Option<&'static str> {
    let rules: [(&str, &str); 15] = [
        ("/costList", "economy stays in the vanilla design (D-085)"),
        (
            "/stuffCategories",
            "economy stays in the vanilla design (D-085)",
        ),
        ("/graphicData/drawSize", "art stays in the vanilla design"),
        ("GunDrawExtension", "art of the gun draw extension"),
        ("UnderBarrel", "weapon platform style conversion"),
        (
            "EquippableAbilityReloadable",
            "weapon platform style conversion",
        ),
        ("compClass", "weapon platform style conversion"),
        (
            "/equippedStatOffsets/MoveSpeed",
            "economy stays in the vanilla design",
        ),
        (
            "AmmoGenPerMagOverride",
            "ammo comp extra without a design field",
        ),
        ("reloadOneAtATime", "ammo comp extra without a design field"),
        (
            "soundCastTail",
            "bow style conversion with its own sound set",
        ),
        ("recoilPattern", "verb field without a design field"),
        ("targetParams", "verb field that Combat Extended adds"),
        (
            "/weaponTags/li",
            "tags beyond the class tag are suggested, not written",
        ),
        ("/tools/", "tool list restructured by Combat Extended"),
    ];
    rules
        .iter()
        .find(|(p, _)| path.contains(p))
        .map(|(_, why)| *why)
}

fn patched_load(
    dir: &std::path::Path,
    types: &Arc<rimstudio_defs::TypeTable>,
    classes: &CeClassNames,
    patch_roots: &[Node],
) -> rimstudio_defs::LoadOutput {
    let mut input = prepare(&official(dir), &version(dir), types.clone());
    let idx = ModIdx(u32::try_from(input.mods.len()).unwrap());
    input
        .mods
        .push(ModEntry::new(idx, "rs.fidelity.patch", "Fidelity patch"));
    for (i, root) in patch_roots.iter().enumerate() {
        input.patch_files.push(PatchFile {
            mod_idx: idx,
            file: FileId(900_000 + u32::try_from(i).unwrap()),
            rel_path: format!("Patches/Fidelity{i}.xml"),
            content: FileContent::parsed(root.clone()),
        });
    }
    input.custom_ops = custom_registry(classes, BTreeMap::new());
    load(input)
}

/// An indented text form of a node, for the dump of one weapon.
fn show(n: &Node, depth: usize, out: &mut String) {
    use std::fmt::Write as _;
    let pad = "  ".repeat(depth);
    let attrs: String = n
        .attrs
        .iter()
        .map(|(k, v)| format!(" {k}=\"{v}\""))
        .collect();
    if n.elements().next().is_none() {
        let _ = writeln!(out, "{pad}<{}{attrs}>{}", n.tag, n.text_content().trim());
        return;
    }
    let _ = writeln!(out, "{pad}<{}{attrs}>", n.tag);
    for c in n.elements() {
        show(c, depth + 1, out);
    }
}

fn dump(title: &str, n: &Node) {
    let mut s = String::new();
    show(n, 1, &mut s);
    println!("---- {title}\n{s}");
}

/// Every class name of Combat Extended that its own data files use in a `Class` attribute.
fn ce_classes_in_use(dir: &std::path::Path) -> BTreeSet<String> {
    fn walk_dir(dir: &std::path::Path, out: &mut BTreeSet<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk_dir(&path, out);
            } else if path.extension().is_some_and(|e| e == "xml")
                && let Ok(text) = std::fs::read_to_string(&path)
            {
                for part in text.split("Class=\"").skip(1) {
                    if let Some(end) = part.find('"') {
                        let class = &part[..end];
                        if class.starts_with("CombatExtended.") {
                            out.insert(class.to_owned());
                        }
                    }
                }
            }
        }
    }
    let mut out = BTreeSet::new();
    for sub in ["Defs", "Patches", "ModPatches"] {
        walk_dir(&dir.join(sub), &mut out);
    }
    out
}

/// Checks that the generated patches only use class names that Combat Extended's own data uses, and prints
/// what the lint says about them.
fn validity_report(
    ce_dir: &std::path::Path,
    cases: &[&Case],
    model: &CeModel,
    classes: &CeClassNames,
) {
    let known = ce_classes_in_use(ce_dir);
    let mut unknown: BTreeSet<String> = BTreeSet::new();
    let mut used: BTreeSet<String> = BTreeSet::new();
    fn visit(
        n: &Node,
        known: &BTreeSet<String>,
        used: &mut BTreeSet<String>,
        unknown: &mut BTreeSet<String>,
    ) {
        if let Some(c) = n.attr("Class").filter(|c| c.starts_with("CombatExtended.")) {
            used.insert(c.to_owned());
            if !known.contains(c) {
                unknown.insert(c.to_owned());
            }
        }
        for e in n.elements() {
            visit(e, known, used, unknown);
        }
    }
    for c in cases {
        for op in &c.patch_ops {
            visit(op, &known, &mut used, &mut unknown);
        }
    }
    println!("\nclasses used by the generated patches: {used:?}");
    println!("classes that Combat Extended's own data does not use: {unknown:?}");
    let file = patch_root(cases, 1);
    let package_id = model
        .names
        .as_ref()
        .map_or_else(String::new, |n| n.package_id.to_lowercase());
    let mut gate = Node::with_text("li", "Compat/CombatExtended");
    gate.set_attr("IfModActive", package_id);
    let mut v16 = Node::new("v1.6");
    v16.push_child(Node::with_text("li", "/"));
    v16.push_child(gate);
    let mut load_folders = Node::new("loadFolders");
    load_folders.push_child(v16);
    let ctx = rimstudio_design::ce::lint::LintContext {
        paths: vec!["Compat/CombatExtended/Patches/Weapons.xml".into()],
        load_folders: Some(load_folders),
        game_version: "1.6".into(),
        ..rimstudio_design::ce::lint::LintContext::default()
    };
    let findings = rimstudio_design::ce::lint::run(&[file], model, &ctx);
    let mut by_code: BTreeMap<String, usize> = BTreeMap::new();
    for d in &findings {
        *by_code.entry(d.code.as_str().to_owned()).or_default() += 1;
    }
    println!("lint of the generated patches: {by_code:?}");
    let _ = classes;
}

fn raw_thing<'a>(raw: &'a [Node], name: &str) -> Option<&'a Node> {
    raw.iter()
        .find(|n| n.tag == THING && n.child_text("defName").map(str::trim) == Some(name))
}

struct Case {
    name: String,
    kind: ItemKind,
    patch_ops: Vec<Node>,
    errors: Vec<String>,
    ce_only_tools: usize,
}

/// Maps the penetration of the real tools onto the tools of the vanilla twin: by label, else by position.
/// Returns the number of real tools that have no vanilla counterpart.
fn map_tools(spec: &mut rimstudio_design::model::DesignSpec) -> usize {
    let Some(ce) = spec.ce.as_mut() else { return 0 };
    let real = std::mem::take(&mut ce.tool_penetration);
    let mut mapped = Vec::new();
    for (i, t) in spec.tools.iter().enumerate() {
        let found = real
            .iter()
            .find(|p| p.tool == t.label)
            .or_else(|| real.get(i));
        if let Some(p) = found {
            mapped.push(CeToolPenetration {
                tool: t.label.clone(),
                sharp: p.sharp,
                blunt: p.blunt,
            });
        }
    }
    let extra = real.len().saturating_sub(spec.tools.len());
    ce.tool_penetration = mapped;
    extra
}

fn build_cases(
    vanilla: &DefDatabases,
    ce: &DefDatabases,
    raw: &[Node],
    model: &CeModel,
) -> (Vec<Case>, Vec<String>) {
    let mut cases = Vec::new();
    let mut skipped = Vec::new();
    let reader = ReaderOptions::default();
    let candidates = scan(raw, vanilla, model);
    let real: BTreeSet<&str> = model
        .guns
        .iter()
        .map(|g| g.def_name.as_str())
        .chain(model.melee.iter().map(|m| m.def_name.as_str()))
        .collect();
    for cand in candidates.iter().filter(|c| real.contains(c.def.as_str())) {
        let name = cand.def.clone();
        if cand.status != ConvertStatus::NotConverted {
            skipped.push(format!("{name}: {:?} ({})", cand.status, cand.reason));
            continue;
        }
        let (Some(v), Some(c)) = (vanilla.get(THING, &name), ce.get(THING, &name)) else {
            skipped.push(format!("{name}: no vanilla twin"));
            continue;
        };
        if let Some(parent) = v.parents.iter().find(|p| real.contains(p.name.as_str())) {
            skipped.push(format!(
                "{name}: inherits the conversion of its parent {}",
                parent.name
            ));
            continue;
        }
        let own = raw_thing(raw, &name);
        let reading = match own {
            Some(node) => spec_from_def_own(
                v,
                vanilla,
                &reader,
                ValueSource::Typed,
                &OwnSource {
                    def: node,
                    projectile: None,
                },
            ),
            None => spec_from_def(v, vanilla, &reader, ValueSource::Typed),
        };
        let Ok(reading) = reading else {
            skipped.push(format!("{name}: the vanilla def cannot be read as a spec"));
            continue;
        };
        let mut spec = reading.spec;
        spec.ce = ce_block_from_def(c, &model.classes, ValueSource::Typed);
        let ce_only_tools = map_tools(&mut spec);
        let container = Container::from_def(
            v,
            raw_thing(raw, &name),
            &model.classes,
            ConversionSource::Unknown,
        );
        let result = match spec.kind {
            ItemKind::Ranged => gun_patch(&spec, model, &container),
            _ => melee_patch(&spec, model, &container),
        };
        let Ok(patch) = result else {
            skipped.push(format!("{name}: generator error"));
            continue;
        };
        let errors: Vec<String> = patch
            .diagnostics
            .iter()
            .filter(|d| d.severity == rimstudio_core::diag::Severity::Error)
            .map(|d| format!("{} {}", d.code.as_str(), d.message))
            .collect();
        cases.push(Case {
            name,
            kind: spec.kind,
            patch_ops: patch.operations,
            errors,
            ce_only_tools,
        });
    }
    (cases, skipped)
}

fn patch_root(cases: &[&Case], times: usize) -> Node {
    let mut root = Node::new("Patch");
    for _ in 0..times {
        for c in cases {
            for op in &c.patch_ops {
                root.push_child(op.clone());
            }
        }
    }
    root
}

#[test]
#[ignore = "needs RIMSTUDIO_GAME_DIR and RIMSTUDIO_CE_DIR"]
fn the_generated_patch_matches_combat_extendeds_own_conversions() {
    let (Some(dir), Some(types)) = (game_dir(), type_table()) else {
        println!("RIMSTUDIO_GAME_DIR or the def type table is missing: skipped");
        return;
    };
    let Some(ce_dir) = std::env::var_os("RIMSTUDIO_CE_DIR").map(PathBuf::from) else {
        println!("RIMSTUDIO_CE_DIR is not set: skipped");
        return;
    };
    let classes = CeClassNames::default();
    let ce_types = Arc::new(with_ce_types(&types, &classes).unwrap());
    let vanilla_input = prepare(&official(&dir), &version(&dir), Arc::new(types.clone()));
    let mut raw = Vec::new();
    for f in &vanilla_input.def_files {
        if let FileContent::Parsed { root } = &f.content {
            raw.extend(root.elements().cloned());
        }
    }
    let vanilla = load(vanilla_input);
    let ce_dir_for_report = ce_dir.clone();
    let mut roots = official(&dir);
    roots.push(ce_dir);
    let mut ce_input = prepare(&roots, &version(&dir), ce_types.clone());
    ce_input.custom_ops = custom_registry(&classes, BTreeMap::new());
    let ce = load(ce_input);
    let model = read_conversions_with(
        &ce.databases,
        &CeReadOptions {
            order: Some(&ce.order),
            vanilla: Some(&vanilla.databases),
            ..CeReadOptions::default()
        },
    );
    assert!(model.is_present());
    let (cases, skipped) = build_cases(&vanilla.databases, &ce.databases, &raw, &model);
    let usable: Vec<&Case> = cases.iter().filter(|c| c.errors.is_empty()).collect();
    println!(
        "{} converted weapons with a twin ({} guns, {} melee), {} generated cleanly, {} skipped",
        cases.len(),
        cases.iter().filter(|c| c.kind == ItemKind::Ranged).count(),
        cases.iter().filter(|c| c.kind == ItemKind::Melee).count(),
        usable.len(),
        skipped.len()
    );
    println!(
        "real conversions where Combat Extended has more tools than the vanilla twin: {} weapons",
        cases.iter().filter(|c| c.ce_only_tools > 0).count()
    );
    for s in &skipped {
        println!("  skipped {s}");
    }
    for c in cases.iter().filter(|c| !c.errors.is_empty()) {
        println!("  generation failed {}: {:?}", c.name, c.errors);
    }
    let once = patched_load(&dir, &ce_types, &classes, &[patch_root(&usable, 1)]);
    let twice = patched_load(&dir, &ce_types, &classes, &[patch_root(&usable, 2)]);
    let failed_ops = once
        .patch_report
        .events
        .iter()
        .filter(|e| !e.result)
        .count();
    println!(
        "patch operations: {} applied, {failed_ops} failed",
        once.patch_report.events.len().saturating_sub(failed_ops)
    );
    for e in once
        .patch_report
        .events
        .iter()
        .filter(|e| !e.result)
        .take(10)
    {
        println!("  failed: {} {:?}", e.description, e.error);
    }
    if let Ok(list) = std::env::var("RIMSTUDIO_FIDELITY_DUMP") {
        for name in list.split(',').filter(|n| !n.is_empty()) {
            println!("\n======== {name}");
            if let Some(n) = raw_thing(&raw, name) {
                dump("raw vanilla", n);
            }
            if let Some(c) = cases.iter().find(|c| c.name == name) {
                for op in &c.patch_ops {
                    dump("our operation", op);
                }
            }
            for (t, dbs) in [
                ("vanilla resolved", &vanilla.databases),
                ("real CE resolved", &ce.databases),
                ("ours resolved", &once.databases),
            ] {
                if let Some(r) = dbs.get(THING, name) {
                    dump(t, &r.node);
                }
            }
        }
    }
    if std::env::var_os("RIMSTUDIO_FIDELITY_TABLE").is_some() {
        for c in &cases {
            let (Some(v), Some(r)) = (
                vanilla.databases.get(THING, &c.name),
                ce.databases.get(THING, &c.name),
            ) else {
                continue;
            };
            let vf = flatten(&v.node);
            let rf = flatten(&r.node);
            let pick = |m: &Pairs, needle: &str| -> String {
                m.iter()
                    .filter(|((p, _), _)| p.contains(needle))
                    .map(|((p, val), n)| {
                        format!(
                            "{}={}{}",
                            p.rsplit('/').next().unwrap_or(p),
                            val,
                            if *n > 1 {
                                format!("x{n}")
                            } else {
                                String::new()
                            }
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",")
            };
            let tags = |m: &Pairs| pick(m, "weaponTags/li");
            println!(
                "TABLE {} | vanilla verb: {} | CE verb: {} | CE firemodes: {} | vanilla tags: {} | CE tags: {}",
                c.name,
                pick(&vf, "verbs/li/burst") + &pick(&vf, "verbs/li/ticks"),
                pick(&rf, "VerbPropertiesCE]/burst") + "," + &pick(&rf, "VerbPropertiesCE]/ticks"),
                pick(&rf, "CompProperties_FireModes]"),
                tags(&vf),
                tags(&rf),
            );
        }
    }
    if std::env::var_os("RIMSTUDIO_FIDELITY_HABITS").is_some() {
        for (kind, labels) in [
            (
                ItemKind::Ranged,
                model
                    .guns
                    .iter()
                    .flat_map(|g| g.tools.iter().map(|t| t.label.clone()))
                    .collect::<BTreeSet<_>>(),
            ),
            (
                ItemKind::Melee,
                model
                    .melee
                    .iter()
                    .flat_map(|m| m.tools.iter().map(|t| t.label.clone()))
                    .collect::<BTreeSet<_>>(),
            ),
        ] {
            for label in labels {
                println!(
                    "HABIT {kind:?} {label}: {:?}",
                    rimstudio_design::ce::patchgen::conventions::label_habit(&model, kind, &label)
                );
            }
        }
    }
    validity_report(&ce_dir_for_report, &usable, &model, &classes);
    let mut findings: Vec<Finding> = Vec::new();
    let mut twice_diff: Vec<String> = Vec::new();
    for c in &usable {
        let (Some(v), Some(r), Some(o)) = (
            vanilla.databases.get(THING, &c.name),
            ce.databases.get(THING, &c.name),
            once.databases.get(THING, &c.name),
        ) else {
            println!("  missing record after load: {}", c.name);
            continue;
        };
        findings.extend(compare(&c.name, &v.node, &r.node, &o.node));
        if let Some(t) = twice.databases.get(THING, &c.name)
            && flatten(&t.node) != flatten(&o.node)
        {
            twice_diff.push(c.name.clone());
        }
    }
    let mut groups: BTreeMap<(Kind, String), Vec<&Finding>> = BTreeMap::new();
    for f in &findings {
        groups
            .entry((f.kind, group_key(&f.path)))
            .or_default()
            .push(f);
    }
    println!("\n== differences by path ({} findings) ==", findings.len());
    for ((kind, path), items) in &groups {
        let weapons: BTreeSet<&str> = items.iter().map(|f| f.weapon.as_str()).collect();
        let sample = items
            .iter()
            .take(3)
            .map(|f| format!("{}: real [{}] ours [{}]", f.weapon, f.real, f.ours))
            .collect::<Vec<_>>()
            .join(" | ");
        println!(
            "{kind:?}\t{path}\tx{} in {} weapons\t{sample}",
            items.len(),
            weapons.len()
        );
    }
    let count = |k: Kind| findings.iter().filter(|f| f.kind == k).count();
    println!(
        "\nTOTAL missing {} not-removed {} value {} extra {} removed-extra {}",
        count(Kind::Missing),
        count(Kind::NotRemoved),
        count(Kind::Value),
        count(Kind::Extra),
        count(Kind::RemovedExtra)
    );
    println!(
        "applied twice differs from applied once: {} of {} weapons",
        twice_diff.len(),
        usable.len()
    );
    let unexplained: Vec<&Finding> = findings
        .iter()
        .filter(|f| matches!(f.kind, Kind::Missing | Kind::NotRemoved))
        .filter(|f| explained(&f.path).is_none())
        .collect();
    println!("unexplained missing or not removed: {}", unexplained.len());
    for f in &unexplained {
        println!("  {:?} {} {}", f.kind, f.weapon, f.path);
    }
    if std::env::var_os("RIMSTUDIO_FIDELITY_STRICT").is_some() {
        assert!(unexplained.is_empty(), "unexplained differences");
        assert_eq!(twice_diff.len(), 0, "applying twice changes a def");
    }
}
