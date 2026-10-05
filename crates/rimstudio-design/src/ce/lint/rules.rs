//! The rule implementations behind [`super::run`].

use std::collections::{BTreeMap, BTreeSet};

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::Node;

use super::codes::{
    CEP001, CEP002, CEP003, CEP004, CEP005, CEP007, CEP008, CEP009, CEP010, CEP011, CEP012, CEP013,
    CEP014, CEP015, CEP016, CEP017, CEP018, CEP020, CEP021, CEP022, CEP023, CEP024, NOT_CHECKED,
    rule_id,
};
use super::xpath::malformed_reason;
use super::{LintContext, LintFile};
use crate::ce::patchgen::folders::{
    IF_MOD_ACTIVE, block_version, ids, normal, suffix_diagnostics, version_numbers,
};
use crate::ce::patchgen::{DEFAULT_CE_NAME, ce_name, ce_package_id};
use crate::ce::reader::{CeModel, MakeGunSpec};
use crate::reader::access::{class_attr, text_of};
use crate::validation::CodeInfo;

/// The namespace prefix of every Combat Extended class.
const CE_NAMESPACE: &str = "CombatExtended.";
/// The class of the vanilla find mod operation.
const FIND_MOD: &str = "PatchOperationFindMod";
/// The class of Combat Extended's own find mod operation.
const CE_FIND_MOD: &str = "CombatExtended.PatchOperationFindMod";
/// The field sections whose names CEP015 checks.
const SECTION_PROPERTIES: &str = "Properties";
const SECTION_AMMO: &str = "AmmoUser";
const SECTION_FIRE: &str = "FireModes";
const SECTION_TOOL: &str = "ToolCE";

type Keyed = (usize, usize, Diagnostic);

struct Cx<'a> {
    model: &'a CeModel,
    ctx: &'a LintContext,
    ce_id: String,
    ce_name: String,
    out: Vec<Keyed>,
    converted: BTreeMap<String, usize>,
    /// How many conditional operations enclose the operation being visited.
    guard_depth: usize,
    seen_ops: BTreeSet<String>,
}

/// The vanilla conditional operation class; an operation under one is considered guarded.
const CONDITIONAL_CLASS: &str = "PatchOperationConditional";

fn same_class(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

fn looks_like_package_id(text: &str) -> bool {
    let parts: Vec<&str> = text.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|p| {
            !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        })
}

fn squash(text: &str) -> String {
    text.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The block of `LoadFolders.xml` that serves the version: the exact one, else the highest older one, else
/// `default`.
fn select_block<'a>(load_folders: &'a Node, version: &str) -> Option<&'a Node> {
    let target = version_numbers(version)?;
    let exact = format!("v{version}");
    if let Some(b) = load_folders.elements().find(|b| b.tag == exact) {
        return Some(b);
    }
    load_folders
        .elements()
        .filter_map(|b| block_version(&b.tag).map(|v| (v, b)))
        .filter(|(v, _)| *v < target)
        .max_by_key(|(v, _)| *v)
        .map(|(_, b)| b)
        .or_else(|| load_folders.elements().find(|b| b.tag == "default"))
}

/// The load folder of a patch file: the path before its `Patches` segment, `/` for the mod root.
fn folder_of(path: &str) -> Option<String> {
    let segments: Vec<&str> = path.split('/').collect();
    let at = segments
        .iter()
        .position(|s| s.eq_ignore_ascii_case("Patches"))?;
    let folder = segments.get(..at)?.join("/");
    Some(normal(&folder))
}

fn pointer_of(parent: &str, tag: &str, index: usize) -> String {
    format!("{parent}/{tag}[{index}]")
}

/// Every element of the subtree with its pointer, in document order.
fn walk<'a>(node: &'a Node, pointer: &str, out: &mut Vec<(String, &'a Node)>) {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for child in node.elements() {
        let n = counts.entry(child.tag.as_str()).or_insert(0);
        *n += 1;
        let p = pointer_of(pointer, &child.tag, *n);
        out.push((p.clone(), child));
        walk(child, &p, out);
    }
}

impl<'a> Cx<'a> {
    fn push(
        &mut self,
        key: (usize, usize),
        info: &CodeInfo,
        path: Option<&str>,
        field: &str,
        args: &[(&str, &str)],
    ) {
        let mut d = info.diagnostic(field, args);
        if let Some(rule) = rule_id(info.code) {
            d = d.with_arg("ruleId", rule);
        }
        if let Some(p) = path {
            d = d.with_arg("path", p);
        }
        self.out.push((key.0, key.1, d));
    }

    fn gated(&self, path: Option<&str>) -> bool {
        let (Some(path), Some(lf)) = (path, self.ctx.load_folders.as_ref()) else {
            return false;
        };
        let (Some(folder), Some(block)) =
            (folder_of(path), select_block(lf, &self.ctx.game_version))
        else {
            return false;
        };
        block.children_named("li").any(|e| {
            normal(&e.text_content()) == folder
                && e.attr(IF_MOD_ACTIVE)
                    .is_some_and(|v| ids(v).any(|i| i.eq_ignore_ascii_case(&self.ce_id)))
        })
    }

    fn file_rules(&mut self, key: (usize, usize), path: &str) {
        let segments: Vec<&str> = path.split('/').collect();
        let fixed: Vec<String> = segments
            .iter()
            .map(|s| {
                if s.eq_ignore_ascii_case("Patches") {
                    "Patches".to_owned()
                } else {
                    (*s).to_owned()
                }
            })
            .collect();
        let mut expected = fixed.join("/");
        if let Some(stem) = expected
            .get(..expected.len().saturating_sub(4))
            .filter(|_| expected.to_ascii_lowercase().ends_with(".xml"))
        {
            expected = format!("{stem}.xml");
        }
        if expected != path {
            self.push(
                key,
                &CEP020,
                Some(path),
                "/path",
                &[("value", path), ("expected", &expected)],
            );
        }
        if let (Some(lf), Some(folder)) = (self.ctx.load_folders.as_ref(), folder_of(path))
            && let Some(block) = select_block(lf, &self.ctx.game_version)
        {
            let loaded = block
                .children_named("li")
                .any(|e| normal(&e.text_content()) == folder);
            if !loaded {
                self.push(
                    key,
                    &CEP018,
                    Some(path),
                    "/path",
                    &[
                        ("folder", &folder),
                        ("version", &self.ctx.game_version.clone()),
                    ],
                );
            }
        }
    }

    fn find_mod_entries(
        &mut self,
        key: (usize, usize),
        path: Option<&str>,
        op: &Node,
        pointer: &str,
    ) {
        let class = class_attr(op).unwrap_or("");
        let mut entries: Vec<(String, String)> = Vec::new();
        if same_class(class, FIND_MOD) {
            if let Some(mods) = op.child("mods") {
                for (i, li) in mods.children_named("li").enumerate() {
                    entries.push((format!("{pointer}/mods/li[{}]", i + 1), li.text_content()));
                }
            }
        } else if same_class(class, CE_FIND_MOD)
            && let Some(m) = op.child("modName")
        {
            entries.push((format!("{pointer}/modName"), m.text_content()));
        }
        for (field, raw) in entries {
            let trimmed = raw.trim();
            if raw != trimmed {
                self.push(
                    key,
                    &CEP003,
                    path,
                    &field,
                    &[("value", &format!("{raw:?}"))],
                );
            }
            if looks_like_package_id(trimmed) {
                self.push(key, &CEP001, path, &field, &[("value", trimmed)]);
            } else if (squash(trimmed) == squash(DEFAULT_CE_NAME)
                || squash(trimmed) == squash(&self.ce_name))
                && trimmed != self.ce_name
            {
                let name = self.ce_name.clone();
                self.push(
                    key,
                    &CEP002,
                    path,
                    &field,
                    &[("value", trimmed), ("name", &name)],
                );
            }
        }
    }

    fn tools(&mut self, key: (usize, usize), path: Option<&str>, op: &Node, pointer: &str) {
        let xpath = op.child_text("xpath").unwrap_or_default().to_owned();
        let Some(value) = op.child("value") else {
            return;
        };
        let mut found: Vec<(String, &Node)> = Vec::new();
        for (i, e) in value.elements().enumerate() {
            if e.tag == "tools" {
                for (j, li) in e.children_named("li").enumerate() {
                    found.push((format!("{pointer}/value/tools/li[{}]", j + 1), li));
                }
            } else if e.tag == "li" && xpath.trim_end().ends_with("/tools") {
                found.push((format!("{pointer}/value/li[{}]", i + 1), e));
            }
        }
        let tool_class = self.model.classes.tool.clone();
        for (field, li) in found {
            let label = text_of_child(li, "label").unwrap_or_else(|| "(no label)".to_owned());
            if class_attr(li).is_some_and(|c| same_class(c, &tool_class)) {
                if li.child("armorPenetrationSharp").is_none()
                    && li.child("armorPenetrationBlunt").is_none()
                {
                    self.push(key, &CEP011, path, &field, &[("tool", &label)]);
                }
            } else {
                self.push(key, &CEP012, path, &field, &[("tool", &label)]);
            }
        }
    }

    fn tag_values(&mut self, key: (usize, usize), path: Option<&str>, tags: &[(String, String)]) {
        if !self.model.is_present() {
            return;
        }
        let prefix = self.model.classes.tag_prefix.clone();
        for (field, tag) in tags {
            if tag.starts_with(&prefix) && !self.model.knows_tag(tag) {
                self.push(key, &CEP016, path, field, &[("value", tag)]);
            }
        }
    }

    fn weapon_tag_additions(
        &mut self,
        key: (usize, usize),
        path: Option<&str>,
        op: &Node,
        pointer: &str,
    ) {
        let xpath = op.child_text("xpath").unwrap_or_default().to_owned();
        let Some(value) = op.child("value") else {
            return;
        };
        let mut tags = Vec::new();
        let at_tags = {
            let x = xpath.trim_end();
            x.ends_with("/weaponTags") || x.ends_with("/weaponTags/li")
        };
        for (i, e) in value.elements().enumerate() {
            if e.tag == "weaponTags" {
                for (j, li) in e.children_named("li").enumerate() {
                    if let Some(t) = text_of(li) {
                        tags.push((format!("{pointer}/value/weaponTags/li[{}]", j + 1), t));
                    }
                }
            } else if at_tags
                && e.tag == "li"
                && let Some(t) = text_of(e)
            {
                tags.push((format!("{pointer}/value/li[{}]", i + 1), t));
            }
        }
        self.tag_values(key, path, &tags);
    }

    fn make_gun(&mut self, key: (usize, usize), path: Option<&str>, op: &Node, pointer: &str) {
        let spec = MakeGunSpec::parse(op);
        if !spec.def_name.is_empty() {
            let n = self.converted.entry(spec.def_name.clone()).or_insert(0);
            *n += 1;
            if *n > 1 {
                self.push(
                    key,
                    &CEP007,
                    path,
                    &format!("{pointer}/defName"),
                    &[("value", &spec.def_name)],
                );
            }
        }
        let def = if spec.def_name.is_empty() {
            "(no def)".to_owned()
        } else {
            spec.def_name.clone()
        };
        if self.guard_depth == 0 {
            self.push(key, &CEP023, path, pointer, &[("def", &def)]);
        }
        let property = |name: &str| -> Option<String> {
            spec.properties
                .iter()
                .find(|n| n.tag == name)
                .and_then(text_of)
        };
        let verb_class = property("verbClass");
        let one_use = verb_class.as_deref().is_some_and(|v| v.contains("OneUse"));
        let mut missing: Vec<(String, String)> = Vec::new();
        if op.child("Properties").is_none() {
            missing.push(("Properties".into(), format!("{pointer}/Properties")));
        } else {
            for name in ["verbClass", "defaultProjectile"] {
                if property(name).is_none() {
                    missing.push((
                        format!("Properties/{name}"),
                        format!("{pointer}/Properties"),
                    ));
                }
            }
        }
        if !one_use {
            match &spec.ammo_user {
                None => missing.push(("AmmoUser".into(), format!("{pointer}/AmmoUser"))),
                Some(children) => {
                    for name in ["ammoSet", "magazineSize"] {
                        if !children
                            .iter()
                            .any(|c| c.tag == name && text_of(c).is_some())
                        {
                            missing
                                .push((format!("AmmoUser/{name}"), format!("{pointer}/AmmoUser")));
                        }
                    }
                }
            }
            if spec.fire_modes.is_none() {
                missing.push(("FireModes".into(), format!("{pointer}/FireModes")));
            }
        }
        let burst = spec
            .properties
            .iter()
            .find(|n| n.tag == "burstShotCount")
            .and_then(text_of)
            .and_then(|t| t.trim().parse::<f64>().ok())
            .filter(|b| *b > 1.0);
        if let (Some(burst), Some(modes)) = (burst, &spec.fire_modes)
            && !modes.iter().any(|c| c.tag == "aimedBurstShotCount")
        {
            self.push(
                key,
                &CEP024,
                path,
                &format!("{pointer}/FireModes"),
                &[("def", &def), ("burst", &burst.to_string())],
            );
        }
        for (what, field) in missing {
            self.push(
                key,
                &CEP008,
                path,
                &field,
                &[("def", &def), ("missing", &what)],
            );
        }
        if let Some(v) = &verb_class
            && !v.starts_with(CE_NAMESPACE)
        {
            self.push(
                key,
                &CEP009,
                path,
                &format!("{pointer}/Properties/verbClass"),
                &[("value", v)],
            );
        }
        if self.model.is_present() {
            let ammo = spec
                .ammo_user
                .as_ref()
                .and_then(|c| c.iter().find(|n| n.tag == "ammoSet").and_then(text_of));
            if let Some(set) = ammo
                && self.model.ammo_set(&set).is_none()
                && !self.ctx.known_defs.contains(&set)
            {
                self.push(
                    key,
                    &CEP013,
                    path,
                    &format!("{pointer}/AmmoUser/ammoSet"),
                    &[("value", &set)],
                );
            }
            if let Some(p) = property("defaultProjectile") {
                let in_ce = self.model.ammo_sets.iter().any(|s| s.has_projectile(&p))
                    || self
                        .model
                        .guns
                        .iter()
                        .any(|g| g.default_projectile.as_deref() == Some(p.as_str()));
                if !in_ce && !self.ctx.known_defs.contains(&p) {
                    self.push(
                        key,
                        &CEP014,
                        path,
                        &format!("{pointer}/Properties/defaultProjectile"),
                        &[("value", &p)],
                    );
                }
            }
        }
        if let Some(known) = self.ctx.known_fields.as_ref() {
            let sections: [(&str, Vec<&Node>); 3] = [
                (SECTION_PROPERTIES, spec.properties.iter().collect()),
                (SECTION_AMMO, spec.ammo_user.iter().flatten().collect()),
                (SECTION_FIRE, spec.fire_modes.iter().flatten().collect()),
            ];
            let mut findings = Vec::new();
            for (section, nodes) in sections {
                if let Some(names) = known.get(section) {
                    for n in nodes {
                        if !names.contains(&n.tag) {
                            findings.push((section, n.tag.clone()));
                        }
                    }
                }
            }
            for (section, name) in findings {
                self.push(
                    key,
                    &CEP015,
                    path,
                    &format!("{pointer}/{section}/{name}"),
                    &[("value", &name), ("section", section)],
                );
            }
        }
        let tags: Vec<(String, String)> = spec
            .weapon_tags
            .iter()
            .enumerate()
            .filter_map(|(i, n)| Some((format!("{pointer}/weaponTags/li[{}]", i + 1), text_of(n)?)))
            .collect();
        self.tag_values(key, path, &tags);
    }

    fn tool_fields(&mut self, key: (usize, usize), path: Option<&str>, op: &Node, pointer: &str) {
        let Some(known) = self
            .ctx
            .known_fields
            .as_ref()
            .and_then(|k| k.get(SECTION_TOOL))
        else {
            return;
        };
        let known = known.clone();
        let tool_class = self.model.classes.tool.clone();
        let mut all = Vec::new();
        walk(op, pointer, &mut all);
        for (p, node) in all {
            if node.tag == "li" && class_attr(node).is_some_and(|c| same_class(c, &tool_class)) {
                for field in node.elements() {
                    if !known.contains(&field.tag) {
                        self.push(
                            key,
                            &CEP015,
                            path,
                            &format!("{p}/{}", field.tag),
                            &[("value", &field.tag), ("section", SECTION_TOOL)],
                        );
                    }
                }
            }
        }
    }

    fn visit(&mut self, key: (usize, usize), path: Option<&str>, op: &Node, pointer: &str) {
        self.find_mod_entries(key, path, op, pointer);
        if let Some(x) = op.child_text("xpath")
            && let Some(reason) = malformed_reason(x)
        {
            self.push(
                key,
                &CEP022,
                path,
                &format!("{pointer}/xpath"),
                &[("value", x), ("reason", &reason)],
            );
        }
        self.tools(key, path, op, pointer);
        self.weapon_tag_additions(key, path, op, pointer);
        self.tool_fields(key, path, op, pointer);
        let class = class_attr(op).unwrap_or("");
        if same_class(class, &self.model.classes.make_gun_op) {
            self.make_gun(key, path, op, pointer);
        }
        let guarded = same_class(class, CONDITIONAL_CLASS);
        self.guard_depth += usize::from(guarded);
        for branch in ["match", "nomatch"] {
            if let Some(b) = op.child(branch) {
                let p = format!("{pointer}/{branch}");
                if b.has_attr("MayRequire") || b.has_attr("MayRequireAnyOf") {
                    self.push(key, &CEP005, path, &p, &[("element", branch)]);
                }
                self.visit(key, path, b, &p);
            }
        }
        self.guard_depth -= usize::from(guarded);
        if let Some(list) = op.child("operations") {
            for (i, li) in list.children_named("li").enumerate() {
                self.visit(
                    key,
                    path,
                    li,
                    &format!("{pointer}/operations/li[{}]", i + 1),
                );
            }
        }
    }

    fn classes_rules(
        &mut self,
        key: (usize, usize),
        path: Option<&str>,
        op: &Node,
        pointer: &str,
        gated: bool,
    ) {
        let mut all = vec![(pointer.to_owned(), op)];
        walk(op, pointer, &mut all);
        let mut flagged_ungated = false;
        for (p, node) in all {
            let Some(class) = class_attr(node) else {
                continue;
            };
            if !class.starts_with(CE_NAMESPACE) {
                continue;
            }
            if !gated && !flagged_ungated {
                flagged_ungated = true;
                self.push(key, &CEP004, path, &p, &[("class", class)]);
            }
            if let Some(types) = self.ctx.ce_types.as_ref()
                && !types.contains(class)
            {
                self.push(key, &CEP010, path, &p, &[("value", class)]);
            }
        }
    }

    fn operation(
        &mut self,
        key: (usize, usize),
        path: Option<&str>,
        op: &Node,
        pointer: &str,
        gated: bool,
    ) {
        if op.has_attr("MayRequire") || op.has_attr("MayRequireAnyOf") {
            self.push(key, &CEP005, path, pointer, &[("element", "the operation")]);
        }
        self.visit(key, path, op, pointer);
        self.classes_rules(key, path, op, pointer, gated);
        let json = serde_json::to_string(op).unwrap_or_default();
        if !self.seen_ops.insert(json) {
            let class = class_attr(op).unwrap_or("").to_owned();
            let xpath = op
                .child_text("xpath")
                .or_else(|| op.child_text("defName"))
                .unwrap_or("")
                .to_owned();
            self.push(
                key,
                &CEP021,
                path,
                pointer,
                &[("class", &class), ("xpath", &xpath)],
            );
        }
    }
}

fn text_of_child(node: &Node, tag: &str) -> Option<String> {
    node.child(tag).and_then(text_of)
}

fn not_checked(cx: &mut Cx<'_>) {
    let mut rules: Vec<(&str, &str)> = Vec::new();
    let absent = "Combat Extended is not installed";
    if !cx.model.is_present() {
        for r in ["CEP010", "CEP013", "CEP014", "CEP015", "CEP016"] {
            rules.push((r, absent));
        }
    } else {
        if cx.ctx.ce_types.is_none() {
            rules.push((
                "CEP010",
                "the type table of the installed Combat Extended was not supplied",
            ));
        }
        if cx.ctx.known_fields.is_none() {
            rules.push((
                "CEP015",
                "the field tables of the installed Combat Extended were not supplied",
            ));
        }
    }
    for (rule, reason) in rules {
        let d = NOT_CHECKED
            .diagnostic("", &[("rule", rule), ("reason", reason)])
            .with_arg("ruleId", rule);
        cx.out.push((0, 0, d));
    }
}

/// Runs every rule over the files.
pub(super) fn run(files: &[LintFile], model: &CeModel, ctx: &LintContext) -> Vec<Diagnostic> {
    let ce_id = ce_package_id(model);
    let ce_name = ce_name(model).to_owned();
    let mut cx = Cx {
        model,
        ctx,
        ce_id,
        ce_name,
        out: Vec::new(),
        converted: BTreeMap::new(),
        guard_depth: 0,
        seen_ops: BTreeSet::new(),
    };
    not_checked(&mut cx);
    if let Some(lf) = ctx.load_folders.as_ref() {
        for d in suffix_diagnostics(lf) {
            cx.out.push((0, 0, d.with_arg("ruleId", "CEP019")));
        }
    }
    for (i, file) in files.iter().enumerate() {
        let fk = i + 1;
        let path = file.path.as_deref();
        if let Some(p) = path {
            cx.file_rules((fk, 0), p);
        }
        let Some(root) = file.root.as_ref() else {
            let reason = file
                .parse_error
                .clone()
                .unwrap_or_else(|| "the file did not parse".to_owned());
            cx.push((fk, 0), &CEP017, path, "", &[("reason", &reason)]);
            continue;
        };
        if root.tag != "Patch" {
            cx.push(
                (fk, 0),
                &CEP017,
                path,
                "/",
                &[(
                    "reason",
                    &format!("the root element is {}, not Patch", root.tag),
                )],
            );
            continue;
        }
        let gated = cx.gated(path);
        let mut op_no = 0usize;
        for child in root.elements() {
            if child.tag != "Operation" {
                cx.push(
                    (fk, 0),
                    &CEP017,
                    path,
                    &format!("/Patch/{}", child.tag),
                    &[(
                        "reason",
                        &format!("the child {} is not an Operation", child.tag),
                    )],
                );
                continue;
            }
            op_no += 1;
            let pointer = pointer_of("/Patch", "Operation", op_no);
            cx.operation((fk, op_no), path, child, &pointer, gated);
        }
    }
    let mut out = cx.out;
    out.sort_by_key(|(f, o, _)| (*f, *o));
    out.into_iter().map(|(_, _, d)| d).collect()
}
