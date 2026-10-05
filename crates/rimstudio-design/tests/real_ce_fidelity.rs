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
use rimstudio_design::ce::suggest::options::accept_options;
use rimstudio_design::model::{CeToolPenetration, CeToolPlan, ItemKind, ValueSource};
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
/// The reason starts with its class: `policy` (a decision of the design, D-085), `optional` (written only
/// when the user accepts a suggestion or gives the node, so the plain conversion leaves it out) or
/// `unsupported` (not expressible). The reasons are the classes of the research notes
/// `ce-structure-fidelity-0.1.0` and `ce-remaining-0.1.0`.
fn explained(path: &str) -> Option<&'static str> {
    let rules: [(&str, &str); 9] = [
        (
            "/costList",
            "policy: economy stays in the vanilla design (D-085)",
        ),
        (
            "/stuffCategories",
            "policy: economy stays in the vanilla design (D-085)",
        ),
        (
            "/graphicData/drawSize",
            "policy: art stays in the vanilla design",
        ),
        (
            "/equippedStatOffsets/MoveSpeed",
            "policy: economy stays in the vanilla design",
        ),
        (
            "GunDrawExtension",
            "optional: art of the gun draw extension, written as a raw node when the user gives it",
        ),
        (
            "soundCastTail",
            "unsupported: bow style conversion with its own sound set",
        ),
        (
            "targetParams",
            "unsupported: verb field that Combat Extended adds",
        ),
        (
            "/weaponTags/li",
            "optional: tags beyond the class tag are written when the user accepts them",
        ),
        (
            "/tools/",
            "optional: a restructured tool list is written when the user accepts the tool plan",
        ),
    ];
    rules
        .iter()
        .find(|(p, _)| path.contains(p))
        .map(|(_, why)| *why)
}

/// The class of a finding that is not a missing or not removed element: extras, removed extras and value
/// differences. A value is a derived number by nature (estimates and vanilla carry overs); an extra is a
/// habit of the generator that the real conversion does not share; both are recorded, none is structure.
fn classify_other(kind: Kind, path: &str) -> Option<&'static str> {
    match kind {
        Kind::Value => {
            Some("derived: a number from the estimators or the vanilla design, not structure")
        }
        Kind::Extra | Kind::RemovedExtra if path.contains("/tools/") => {
            Some("habit: a tool habit or a tool plan entry that the real conversion does not have")
        }
        Kind::Extra
            if path.contains("CompProperties_FireModes") || path.contains("burstShotCount") =>
        {
            Some(
                "habit: fire modes and burst follow the class of the converted guns or the vanilla burst",
            )
        }
        Kind::Extra if path.contains("recoilAmount") => {
            Some("habit: recoil of the class estimate where the real conversion writes none")
        }
        Kind::RemovedExtra if path.contains("/weaponTags/") => Some(
            "unsupported: the real conversion removes a vanilla tag, which the block cannot say",
        ),
        _ => None,
    }
}

/// [`explained`] with the kind of the finding: a vanilla tag that the real conversion removes and ours keeps
/// is a tag removal that the block cannot say, not a tag the user could add.
fn explained_for(kind: Kind, path: &str) -> Option<&'static str> {
    if kind == Kind::NotRemoved && path.contains("/weaponTags/li") {
        return Some(
            "unsupported: the real conversion removes a vanilla tag, which the block cannot say",
        );
    }
    explained(path)
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
    /// The weapon went through the bow conversion.
    bow: bool,
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

/// How the optional additions of the block are filled, from `RIMSTUDIO_FIDELITY_PLAN`:
/// `none` writes the plain conversion only; `suggested` (the default) accepts every suggestion of the
/// user's conversions (companion tags, tool plan, recoil pattern, reload); `explicit` writes what the real
/// conversion did as the user would state it: a tool plan from the real tools, the tags and the extra
/// `modExtensions` entries as raw nodes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PlanMode {
    None,
    Suggested,
    Explicit,
}

fn plan_mode() -> PlanMode {
    match std::env::var("RIMSTUDIO_FIDELITY_PLAN").as_deref() {
        Ok("none") => PlanMode::None,
        Ok("explicit") => PlanMode::Explicit,
        _ => PlanMode::Suggested,
    }
}

/// The tool plan that states the real tools of a conversion: every field of the real tool, starting from
/// the vanilla tool with the same label, else the one at the same position.
fn explicit_plan(
    rows: &[rimstudio_design::ce::reader::CeToolRow],
    vanilla_tools: &[rimstudio_design::model::ToolSpec],
) -> Vec<CeToolPlan> {
    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            let from = vanilla_tools
                .iter()
                .find(|t| t.label == r.label)
                .or_else(|| vanilla_tools.get(i))
                .map(|t| t.label.clone())
                .filter(|l| *l != r.label);
            CeToolPlan {
                label: r.label.clone(),
                from,
                capacities: Some(r.capacities.clone()),
                power: r.power,
                cooldown: r.cooldown,
                chance_factor: r.chance_factor,
                armor_penetration_sharp: r.ap_sharp,
                armor_penetration_blunt: r.ap_blunt,
                linked_body_parts_group: r.linked_body_parts_group.clone(),
            }
        })
        .collect()
}

/// The `modExtensions` entries of the real def that the vanilla def does not have, as one raw node.
fn extra_mod_extensions(vanilla: &Node, real: &Node) -> Option<Node> {
    let have: Vec<String> = vanilla
        .child("modExtensions")
        .map(|m| {
            m.children_named("li")
                .filter_map(|l| l.attr("Class").map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    let mut out = Node::new("modExtensions");
    for li in real.child("modExtensions")?.children_named("li") {
        if li
            .attr("Class")
            .is_some_and(|c| !have.contains(&c.to_owned()))
        {
            out.push_child(li.clone());
        }
    }
    let any = out.elements().next().is_some();
    any.then_some(out)
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
        match plan_mode() {
            PlanMode::None => {}
            PlanMode::Suggested => {
                spec = accept_options(&spec, model, None).spec;
            }
            PlanMode::Explicit => {
                let rows = model
                    .guns
                    .iter()
                    .find(|g| g.def_name == name)
                    .map(|g| g.tools.clone())
                    .or_else(|| {
                        model
                            .melee
                            .iter()
                            .find(|m| m.def_name == name)
                            .map(|m| m.tools.clone())
                    })
                    .unwrap_or_default();
                let tags: Vec<String> = model
                    .guns
                    .iter()
                    .find(|g| g.def_name == name)
                    .map(|g| (g.weapon_tags.clone(), g.twin_tags.clone()))
                    .or_else(|| {
                        model
                            .melee
                            .iter()
                            .find(|m| m.def_name == name)
                            .map(|m| (m.weapon_tags.clone(), m.twin_tags.clone()))
                    })
                    .map(|(all, twin)| all.into_iter().filter(|t| !twin.contains(t)).collect())
                    .unwrap_or_default();
                let vanilla_tools = spec.tools.clone();
                if let Some(ce) = spec.ce.as_mut() {
                    ce.tool_plan = explicit_plan(&rows, &vanilla_tools);
                    ce.extra_tags = tags;
                    ce.raw_extras = extra_mod_extensions(&v.node, &c.node).into_iter().collect();
                }
            }
        }
        let container = Container::from_def(
            v,
            raw_thing(raw, &name),
            &model.classes,
            ConversionSource::Unknown,
        );
        let bow = rimstudio_design::ce::patchgen::is_bow_spec(&spec);
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
            bow,
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
    if std::env::var_os("RIMSTUDIO_FIDELITY_HABITS").is_some() {
        use rimstudio_design::ce::patchgen::conventions as conv;
        for kind in [ItemKind::Ranged, ItemKind::Melee] {
            println!(
                "DROPPED {kind:?}: {:?}",
                conv::dropped_tool_fields(&model, kind)
            );
        }
        for class in &model.ai_class_tags {
            println!(
                "CLASS {class}: tags {:?} recoil {:?} reload {:?}",
                conv::companion_shares(&model, class),
                conv::recoil_pattern_habit(&model, class),
                conv::reload_one_at_a_time_habit(&model, class)
            );
        }
        println!(
            "TWIN TOOL FIELDS {:?}",
            model
                .extras
                .guns
                .iter()
                .chain(model.extras.melee.iter())
                .map(|e| (
                    e.def_name.clone(),
                    e.twin_tool_fields.clone(),
                    e.tool_fields.clone()
                ))
                .filter(|(_, t, _)| !t.is_empty())
                .collect::<Vec<_>>()
        );
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
    platform_report(&PlatformRun {
        cases: &cases,
        vanilla: &vanilla.databases,
        real: &ce.databases,
        once: &once.databases,
        twice: &twice.databases,
        model: &model,
        raw: &raw,
    });
    bow_convert_report(&vanilla.databases, &model, &raw, &cases);
    bow_report(&BowRun {
        cases: &cases,
        vanilla: &vanilla.databases,
        real: &ce.databases,
        once: &once.databases,
        twice: &twice.databases,
        model: &model,
    });
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
    let mut by_class: BTreeMap<(&str, &str), usize> = BTreeMap::new();
    for f in &findings {
        let why = match f.kind {
            Kind::Missing | Kind::NotRemoved => explained_for(f.kind, &f.path),
            other => classify_other(other, &f.path),
        };
        let class = why
            .and_then(|why| why.split(':').next())
            .unwrap_or("unclassified");
        let kind = match f.kind {
            Kind::Missing => "missing",
            Kind::NotRemoved => "not removed",
            Kind::RemovedExtra => "removed extra",
            Kind::Extra => "extra",
            Kind::Value => "value",
        };
        *by_class.entry((class, kind)).or_default() += 1;
    }
    println!("plan mode: {:?}", plan_mode());
    for ((class, kind), n) in &by_class {
        println!("CLASS {class}\t{kind}\t{n}");
    }
    let unclassified: usize = by_class
        .iter()
        .filter(|((c, _), _)| *c == "unclassified")
        .map(|(_, n)| n)
        .sum();
    println!("unclassified findings of any kind: {unclassified}");
    println!("unexplained missing or not removed: {}", unexplained.len());
    for f in &unexplained {
        println!("  {:?} {} {}", f.kind, f.weapon, f.path);
    }
    if std::env::var_os("RIMSTUDIO_FIDELITY_STRICT").is_some() {
        assert!(unexplained.is_empty(), "unexplained differences");
        assert_eq!(unclassified, 0, "findings without a class");
        assert_eq!(twice_diff.len(), 0, "applying twice changes a def");
    }
}

/// What the bow report needs from the run.
struct BowRun<'a> {
    cases: &'a [Case],
    vanilla: &'a DefDatabases,
    real: &'a DefDatabases,
    once: &'a rimstudio_defs::DefDatabases,
    twice: &'a rimstudio_defs::DefDatabases,
    model: &'a CeModel,
}

/// The bow part of the run: the findings of the bow conversions alone (the weapons that went through the
/// bow style), whether applying the patch twice changes them, and the variants that inherit a bow's
/// conversion (compared as resolved defs, they carry no patch of their own).
fn bow_report(run: &BowRun<'_>) {
    let bows: Vec<&Case> = run
        .cases
        .iter()
        .filter(|c| c.bow && c.errors.is_empty())
        .collect();
    println!("\n== bows ==");
    println!(
        "converted bows in the library (examples of the bow estimate): {}",
        run.model.bows().count()
    );
    println!("bow conversions compared: {}", bows.len());
    let mut findings: Vec<Finding> = Vec::new();
    let mut twice_diff = 0usize;
    for c in &bows {
        let (Some(v), Some(r), Some(o)) = (
            run.vanilla.get(THING, &c.name),
            run.real.get(THING, &c.name),
            run.once.get(THING, &c.name),
        ) else {
            continue;
        };
        findings.extend(compare(&c.name, &v.node, &r.node, &o.node));
        if run
            .twice
            .get(THING, &c.name)
            .is_some_and(|t| flatten(&t.node) != flatten(&o.node))
        {
            twice_diff += 1;
        }
    }
    let inheritors: Vec<String> = run
        .model
        .guns
        .iter()
        .filter(|g| g.bow)
        .filter(|g| bows.iter().all(|c| c.name != g.def_name))
        .filter(|g| {
            run.vanilla.get(THING, &g.def_name).is_some_and(|v| {
                v.parents
                    .iter()
                    .any(|p| bows.iter().any(|c| c.name == p.name))
            })
        })
        .map(|g| g.def_name.clone())
        .collect();
    let mut inherited: Vec<Finding> = Vec::new();
    for name in &inheritors {
        if let (Some(v), Some(r), Some(o)) = (
            run.vanilla.get(THING, name),
            run.real.get(THING, name),
            run.once.get(THING, name),
        ) {
            inherited.extend(compare(name, &v.node, &r.node, &o.node));
        }
    }
    println!(
        "variants that inherit a bow conversion: {} ({})",
        inheritors.len(),
        inheritors.join(", ")
    );
    let count = |fs: &[Finding], k: Kind| fs.iter().filter(|f| f.kind == k).count();
    // A variant that lists its own components (`Inherit="False"`) does not receive the components its
    // parent's conversion adds; Combat Extended converts such variants in a separate file.
    let explained_for = |label: &str, path: &str| {
        explained(path).is_some()
            || (label == "inherited" && path.starts_with("/comps/li[@Class=CombatExtended."))
    };
    for (label, fs) in [("own", &findings), ("inherited", &inherited)] {
        println!(
            "BOW {label} missing {} not-removed {} value {} extra {} removed-extra {}",
            count(fs, Kind::Missing),
            count(fs, Kind::NotRemoved),
            count(fs, Kind::Value),
            count(fs, Kind::Extra),
            count(fs, Kind::RemovedExtra)
        );
        let unexplained = fs
            .iter()
            .filter(|f| matches!(f.kind, Kind::Missing | Kind::NotRemoved))
            .filter(|f| !explained_for(label, &f.path))
            .count();
        println!("BOW {label} unexplained missing or not removed: {unexplained}");
    }
    println!(
        "BOW applied twice differs from applied once: {twice_diff} of {}",
        bows.len()
    );
    let mut by_group: BTreeMap<(Kind, String), usize> = BTreeMap::new();
    for f in findings.iter().chain(inherited.iter()) {
        *by_group.entry((f.kind, group_key(&f.path))).or_default() += 1;
    }
    for ((kind, path), n) in &by_group {
        println!("BOW {kind:?}\t{path}\tx{n}");
    }
    if std::env::var_os("RIMSTUDIO_FIDELITY_STRICT").is_some() {
        assert!(
            findings
                .iter()
                .filter(|f| matches!(f.kind, Kind::Missing | Kind::NotRemoved))
                .all(|f| explained_for("own", &f.path))
                && inherited
                    .iter()
                    .filter(|f| matches!(f.kind, Kind::Missing | Kind::NotRemoved))
                    .all(|f| explained_for("inherited", &f.path)),
            "unexplained bow differences"
        );
        assert_eq!(twice_diff, 0, "applying a bow patch twice changes a def");
    }
}

/// The convert flow on the real vanilla bows: what it asks with no answers, what is left to ask once the
/// arrow set is answered, and whether the plan of a fully answered conversion lints without findings that
/// are not about the missing Run and Gun mod. Prints; asserts the structure only.
fn bow_convert_report(vanilla: &DefDatabases, model: &CeModel, raw: &[Node], cases: &[Case]) {
    use rimstudio_design::ce::patchgen::{CeProjectState, ConvertAnswers, ConvertEnv, convert};
    use rimstudio_design::plan::ProjectLayout;
    let layout = ProjectLayout::default();
    let reader = ReaderOptions::default();
    let state = CeProjectState::default();
    let source = ConversionSource::Unknown;
    let env = ConvertEnv {
        dbs: vanilla,
        model,
        layout: &layout,
        reader: &reader,
        project: raw,
        state: &state,
        source: &source,
    };
    let candidates = scan(raw, vanilla, model);
    for case in cases.iter().filter(|c| c.bow) {
        let Some(candidate) = candidates.iter().find(|c| c.def == case.name) else {
            continue;
        };
        let Some(real) = model.gun(&case.name) else {
            continue;
        };
        let fields = |a: &rimstudio_design::ce::patchgen::AskList| {
            a.items.iter().map(|i| i.field.clone()).collect::<Vec<_>>()
        };
        let bare = convert(candidate, &ConvertAnswers::default(), &env);
        println!(
            "BOW CONVERT {} asks with no answers: {:?}",
            case.name,
            fields(&bare.asks)
        );
        let answers = ConvertAnswers {
            ammo_set: real.ammo_set.clone(),
            ..ConvertAnswers::default()
        };
        let set_answered = convert(candidate, &answers, &env);
        println!(
            "BOW CONVERT {} asks after the arrow set: {:?}; derived {:?}",
            case.name,
            fields(&set_answered.asks),
            set_answered
                .derived
                .iter()
                .map(|d| format!("{}={}", d.field, d.value))
                .collect::<Vec<_>>()
        );
        // Answer every open number with the real conversion's own value and look at the plan.
        let mut full = answers.clone();
        for ask in &set_answered.asks.items {
            let real_number = |pointer: &str| -> Option<f64> {
                match pointer {
                    "/ce/bulk" => real.stats.get("bulk").copied(),
                    "/ce/swayFactor" => real.stats.get("sway").copied(),
                    "/ce/shotSpread" => real.stats.get("spread").copied(),
                    _ => None,
                }
            };
            if let Some(v) = real_number(&ask.field) {
                let s = Some(rimstudio_design::model::Sourced::new(v, ValueSource::Typed));
                match ask.field.as_str() {
                    "/ce/bulk" => full.overrides.bulk = s,
                    "/ce/swayFactor" => full.overrides.sway_factor = s,
                    _ => full.overrides.shot_spread = s,
                }
            } else if let Some(rest) = ask.field.strip_prefix("/ce/toolPenetration/")
                && let Some((label, _)) = rest.rsplit_once('/')
            {
                full.tool_penetration.push(CeToolPenetration {
                    tool: label.to_owned(),
                    sharp: None,
                    blunt: real
                        .tools
                        .first()
                        .and_then(|t| t.ap_blunt)
                        .map(|v| rimstudio_design::model::Sourced::new(v, ValueSource::Typed)),
                });
            }
        }
        let done = convert(candidate, &full, &env);
        println!(
            "BOW CONVERT {} fully answered: asks {:?}, plan files {}, errors {}",
            case.name,
            fields(&done.asks),
            done.plan.files.len(),
            done.plan
                .diagnostics
                .iter()
                .filter(|d| d.severity == rimstudio_core::diag::Severity::Error)
                .count()
        );
        if std::env::var_os("RIMSTUDIO_FIDELITY_STRICT").is_some() {
            assert!(done.asks.is_empty(), "{}: asks remain", case.name);
            assert!(!done.plan.files.is_empty(), "{}: no plan", case.name);
        }
    }
}

/// What the platform report needs from the run.
struct PlatformRun<'a> {
    cases: &'a [Case],
    vanilla: &'a DefDatabases,
    real: &'a DefDatabases,
    once: &'a rimstudio_defs::DefDatabases,
    twice: &'a rimstudio_defs::DefDatabases,
    model: &'a CeModel,
    raw: &'a [Node],
}

/// The platform part of the run: the weapons whose real conversion carries an under barrel unit, compared
/// like every other weapon (their findings are counted separately, so the numbers before and after the
/// platform work can be told apart), plus what the convert flow asks for such a weapon and what the install
/// has of weapon platforms and attachment defs.
fn platform_report(run: &PlatformRun<'_>) {
    use rimstudio_design::ce::patchgen::{CeProjectState, ConvertAnswers, ConvertEnv, convert};
    use rimstudio_design::plan::ProjectLayout;
    let classes = &run.model.classes;
    let family: Vec<&Case> = run
        .cases
        .iter()
        .filter(|c| c.errors.is_empty())
        .filter(|c| {
            run.real.get(THING, &c.name).is_some_and(|r| {
                rimstudio_design::ce::reader::platform::under_barrel_comp(&r.node, classes)
                    .is_some()
            })
        })
        .collect();
    println!("\n== platforms and under barrel units ==");
    println!(
        "install: {} weapon platforms, {} attachment defs, {} under barrel units with data",
        run.model.platform.platforms.len(),
        run.model.platform.attachments.len(),
        run.model.platform.under_barrels.len()
    );
    println!(
        "weapons whose real conversion has an under barrel unit and a vanilla twin: {} ({})",
        family.len(),
        family
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut findings: Vec<Finding> = Vec::new();
    let mut twice_diff = 0usize;
    for c in &family {
        let (Some(v), Some(r), Some(o)) = (
            run.vanilla.get(THING, &c.name),
            run.real.get(THING, &c.name),
            run.once.get(THING, &c.name),
        ) else {
            continue;
        };
        findings.extend(compare(&c.name, &v.node, &r.node, &o.node));
        if run
            .twice
            .get(THING, &c.name)
            .is_some_and(|t| flatten(&t.node) != flatten(&o.node))
        {
            twice_diff += 1;
        }
    }
    let count = |k: Kind| findings.iter().filter(|f| f.kind == k).count();
    let unit_paths = |f: &Finding| {
        f.path.contains("UnderBarrel")
            || f.path.contains("EquippableAbility")
            || f.path.contains("compClass")
    };
    println!(
        "PLATFORM missing {} not-removed {} value {} extra {} removed-extra {}",
        count(Kind::Missing),
        count(Kind::NotRemoved),
        count(Kind::Value),
        count(Kind::Extra),
        count(Kind::RemovedExtra)
    );
    let on_unit: Vec<&Finding> = findings
        .iter()
        .filter(|f| matches!(f.kind, Kind::Missing | Kind::NotRemoved | Kind::Value))
        .filter(|f| unit_paths(f))
        .collect();
    println!(
        "PLATFORM findings on the unit and its components (under barrel, ability component, component classes): {}",
        on_unit.len()
    );
    for f in &on_unit {
        println!(
            "  {:?} {} {} real [{}] ours [{}]",
            f.kind, f.weapon, f.path, f.real, f.ours
        );
    }
    let unexplained = findings
        .iter()
        .filter(|f| matches!(f.kind, Kind::Missing | Kind::NotRemoved))
        .filter(|f| explained(&f.path).is_none())
        .count();
    println!("PLATFORM unexplained missing or not removed: {unexplained}");
    println!(
        "PLATFORM applied twice differs from applied once: {twice_diff} of {}",
        family.len()
    );
    // The convert flow on the vanilla weapons that carry a unit of their own.
    let layout = ProjectLayout::default();
    let reader = ReaderOptions::default();
    let state = CeProjectState::default();
    let source = ConversionSource::Unknown;
    let env = ConvertEnv {
        dbs: run.vanilla,
        model: run.model,
        layout: &layout,
        reader: &reader,
        project: run.raw,
        state: &state,
        source: &source,
    };
    let candidates = scan(run.raw, run.vanilla, run.model);
    for c in &family {
        let Some(candidate) = candidates.iter().find(|x| x.def == c.name) else {
            continue;
        };
        println!(
            "PLATFORM CONVERT {} status {:?}: {}",
            c.name, candidate.status, candidate.reason
        );
        let out = convert(candidate, &ConvertAnswers::default(), &env);
        println!(
            "PLATFORM CONVERT {} asks with no answers: {:?}",
            c.name,
            out.asks
                .items
                .iter()
                .map(|i| i.field.clone())
                .collect::<Vec<_>>()
        );
    }
    // Answered with the real numbers and the real unit, the conversion produces a plan without asks.
    for c in &family {
        let (Some(candidate), Some(real), Some(entry), Some(twin)) = (
            candidates.iter().find(|x| x.def == c.name),
            run.model.gun(&c.name),
            run.model
                .platform
                .under_barrels
                .iter()
                .find(|e| e.def_name == c.name),
            run.vanilla.get(THING, &c.name),
        ) else {
            continue;
        };
        let typed = |v: Option<f64>| {
            v.map(|v| rimstudio_design::model::Sourced::new(v, ValueSource::Typed))
        };
        let mut answers = ConvertAnswers {
            ammo_set: real.ammo_set.clone(),
            weapon_tag_class: real.ai_class.clone(),
            one_handed: Some(false),
            belt_fed: Some(false),
            ..ConvertAnswers::default()
        };
        answers.overrides.bulk = typed(real.stats.get("bulk").copied());
        answers.overrides.sway_factor = typed(real.stats.get("sway").copied());
        answers.overrides.shot_spread = typed(real.stats.get("spread").copied());
        answers.overrides.magazine_size = real
            .stats
            .get("magazine")
            .map(|m| rimstudio_design::model::Sourced::new(*m as u32, ValueSource::Typed));
        answers.overrides.reload_time = typed(real.stats.get("reload").copied());
        answers.overrides.under_barrel = Some(entry.unit.clone());
        let labels: Vec<String> = twin
            .node
            .child("tools")
            .map(|t| {
                t.children_named("li")
                    .filter_map(|l| l.child_text("label").map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default();
        for (i, label) in labels.iter().enumerate() {
            answers.tool_penetration.push(CeToolPenetration {
                tool: label.clone(),
                sharp: None,
                blunt: typed(real.tools.get(i).and_then(|t| t.ap_blunt)),
            });
        }
        let done = convert(candidate, &answers, &env);
        let errors = done
            .plan
            .diagnostics
            .iter()
            .filter(|d| d.severity == rimstudio_core::diag::Severity::Error)
            .count();
        println!(
            "PLATFORM CONVERT {} fully answered: asks {:?}, plan files {}, errors {errors}",
            c.name,
            done.asks
                .items
                .iter()
                .map(|i| i.field.clone())
                .collect::<Vec<_>>(),
            done.plan.files.len()
        );
        if std::env::var_os("RIMSTUDIO_FIDELITY_STRICT").is_some() {
            assert!(done.asks.is_empty(), "{}: asks remain", c.name);
            assert!(!done.plan.files.is_empty(), "{}: no plan", c.name);
            assert_eq!(errors, 0, "{}: plan errors", c.name);
        }
    }
    for name in ["Gun_Incinerator"] {
        if let Some(candidate) = candidates.iter().find(|x| x.def == name) {
            println!(
                "PLATFORM SCAN {name}: {:?} ({})",
                candidate.status, candidate.reason
            );
        }
    }
    if std::env::var_os("RIMSTUDIO_FIDELITY_STRICT").is_some() {
        assert!(
            on_unit.is_empty(),
            "the unit of a platform family weapon differs"
        );
        assert_eq!(
            twice_diff, 0,
            "applying a platform patch twice changes a def"
        );
    }
}
