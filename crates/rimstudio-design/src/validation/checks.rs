//! The pure validation functions of the designer.
//!
//! Everything here takes a shared reference and returns diagnostics: nothing is clamped, corrected or
//! normalised (IT-042). Only structural impossibilities are errors (IT-041): a missing required field, a
//! value that is not finite or negative where it cannot be, a name the game rejects, text XML cannot hold,
//! and a Combat Extended class inside the vanilla definition. Everything else is a warning or an info.
//!
//! Checks that need data this crate does not have (peer pools, bands, an install, CE data) live elsewhere;
//! [`DefLookup`] and [`validate_refs`] are the seam for the install-backed reference checks.
//!
//! Two scopes exist because the vanilla definition and the optional CE patch are written separately:
//! [`validate_vanilla`] never looks at the CE block, [`validate_ce_patch`] looks only at it.

use std::collections::BTreeSet;

use rimstudio_core::diag::Diagnostic;

use crate::armor::MAX_RATING;
use rimstudio_core::tree::Node;

use crate::model::{
    CePatchSpec, DesignSpec, Draft, INHERIT_RESETTABLE, ItemKind, MODELLED_THING_FIELDS,
    ProjectileChoice, ScalarField, Sourced,
};

use super::codes::{self, CodeInfo};

/// Ranges above this many cells look like a different unit. A sanity limit, not a game value.
pub const MAX_PLAUSIBLE_RANGE: f64 = 100.0;
/// Times above this many seconds look like ticks. A sanity limit, not a game value.
pub const MAX_PLAUSIBLE_SECONDS: f64 = 60.0;
/// Masses above this many kilograms look like a different unit. A sanity limit, not a game value.
pub const MAX_PLAUSIBLE_MASS: f64 = 100.0;

/// The names the game refuses for a def.
const RESERVED_DEF_NAMES: [&str; 2] = ["null", "UnnamedDef"];

/// The text that marks a Combat Extended class.
const CE_CLASS_MARK: &str = "CombatExtended";

/// Which part of the spec a pass looks at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    Vanilla,
    Ce,
}

struct Ctx<'a> {
    spec: &'a DesignSpec,
    scope: Scope,
    out: Vec<Diagnostic>,
}

impl Ctx<'_> {
    fn emit(&mut self, info: CodeInfo, field: &str, args: &[(&str, &str)]) {
        self.out.push(info.diagnostic(field, args));
    }

    fn missing(&mut self, field: &str, label: &str) {
        if self.spec.accepted_missing.iter().any(|f| f == field) {
            self.emit(codes::ACCEPTED_MISSING, field, &[("label", label)]);
        } else {
            self.emit(codes::REQUIRED_MISSING, field, &[("label", label)]);
        }
    }

    fn invalid_value(&mut self, field: &str, label: &str, value: f64, reason: &str) {
        let text = crate::model::format_number(value);
        self.emit(
            codes::VALUE_INVALID,
            field,
            &[("label", label), ("value", &text), ("reason", reason)],
        );
    }

    /// Checks a def name token. An empty value is the business of the required check, not of this one.
    fn check_token(&mut self, field: &str, label: &str, value: &str, reserved: bool) {
        if value.is_empty() {
            return;
        }
        if let Some(reason) = token_problem(value, reserved) {
            self.emit(
                codes::NAME_INVALID,
                field,
                &[("label", label), ("value", value), ("reason", reason)],
            );
        }
    }
}

impl Ctx<'_> {
    /// Like [`Ctx::check_token`] for a name that becomes an XML element name (ingredients, stat names).
    fn check_element_token(&mut self, field: &str, label: &str, value: &str) {
        if value.is_empty() {
            return;
        }
        let problem = token_problem(value, false).or_else(|| {
            let first = value.chars().next()?;
            (!(first.is_ascii_alphabetic() || first == '_')).then_some(
                "it becomes an XML element name and must start with a letter or underscore",
            )
        });
        if let Some(reason) = problem {
            self.emit(
                codes::NAME_INVALID,
                field,
                &[("label", label), ("value", value), ("reason", reason)],
            );
        }
    }
}

fn token_problem(value: &str, reserved: bool) -> Option<&'static str> {
    if !value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Some("only letters, digits, underscores and dashes are allowed");
    }
    if reserved && RESERVED_DEF_NAMES.contains(&value) {
        return Some("the game reserves this name");
    }
    None
}

fn val(slot: Option<Sourced<f64>>) -> Option<f64> {
    slot.map(|s| s.value)
}

fn nonempty(text: &str) -> bool {
    !text.trim().is_empty()
}

fn allows_negative(field: ScalarField) -> bool {
    matches!(
        field,
        ScalarField::CeMeleeCritChance
            | ScalarField::CeMeleeParryChance
            | ScalarField::CeMeleeDodgeChance
            | ScalarField::CeParryBonus
    )
}

fn is_whole(value: f64) -> bool {
    value.is_finite() && value.fract() == 0.0
}

// ---------------------------------------------------------------------------------------------------------
// public entry points

/// Validates the vanilla definition inputs: required fields, value sanity, names, text, units, tools and
/// the vanilla definition rules. Never looks at the CE block. Pure.
#[must_use]
pub fn validate_vanilla(spec: &DesignSpec) -> Vec<Diagnostic> {
    let mut ctx = Ctx {
        spec,
        scope: Scope::Vanilla,
        out: Vec::new(),
    };
    required_vanilla(&mut ctx);
    values(&mut ctx);
    names_vanilla(&mut ctx);
    strings(&mut ctx);
    units_vanilla(&mut ctx);
    tools(&mut ctx);
    lists(&mut ctx);
    vanilla_rules(&mut ctx);
    carried(&mut ctx);
    ctx.out
}

/// Validates the optional Combat Extended inputs when the patch is on: the required fields of the kind
/// (IT-030), value sanity, names and units. Returns nothing when the CE block is absent. CE data dependent
/// rules (ammo set membership, ranges from the CE pools) are not here. Pure.
#[must_use]
pub fn validate_ce_patch(spec: &DesignSpec) -> Vec<Diagnostic> {
    let Some(ce) = spec.ce.as_ref() else {
        return Vec::new();
    };
    let mut ctx = Ctx {
        spec,
        scope: Scope::Ce,
        out: Vec::new(),
    };
    required_ce(&mut ctx, ce);
    values(&mut ctx);
    names_ce(&mut ctx, ce);
    strings(&mut ctx);
    units_ce(&mut ctx);
    ctx.out
}

/// Validates the whole spec: [`validate_vanilla`] followed by [`validate_ce_patch`].
#[must_use]
pub fn validate_spec(spec: &DesignSpec) -> Vec<Diagnostic> {
    let mut out = validate_vanilla(spec);
    out.extend(validate_ce_patch(spec));
    out
}

/// Validates a draft: the spec plus the consistency of the envelope. Pointers stay relative to `spec`.
#[must_use]
pub fn validate_draft(draft: &Draft) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if !draft.is_consistent() {
        out.push(codes::DRAFT_INCONSISTENT.diagnostic(
            "/kind",
            &[
                ("draft", draft.kind.as_str()),
                ("spec", draft.spec.kind.as_str()),
            ],
        ));
    }
    out.extend(validate_spec(&draft.spec));
    out
}

// ---------------------------------------------------------------------------------------------------------
// required fields (IT-030)

fn required_vanilla(ctx: &mut Ctx<'_>) {
    let spec = ctx.spec;
    if !nonempty(&spec.identity.def_name) {
        ctx.missing("/identity/defName", "def name");
    }
    if !nonempty(&spec.identity.label) {
        ctx.missing("/identity/label", "label");
    }
    if !spec.parent.as_ref().is_some_and(|p| nonempty(&p.def_name)) {
        ctx.missing("/parent", "parent base");
    }
    if spec.tech_level.is_none() {
        ctx.missing("/techLevel", "tier");
    }
    if !spec.role.as_deref().is_some_and(nonempty) {
        ctx.missing("/role", "role");
    }
    if spec.mass.is_none() {
        ctx.missing("/mass", "mass");
    }
    if spec.work_to_make.is_none() {
        ctx.missing("/workToMake", "work to make");
    }
    match spec.kind {
        ItemKind::Ranged => required_ranged(ctx),
        ItemKind::Melee => required_melee(ctx),
    }
    for (i, tool) in spec.tools.iter().enumerate() {
        let n = i + 1;
        if !nonempty(&tool.label) {
            ctx.missing(&format!("/tools/{i}/label"), &format!("tool {n} label"));
        }
        if tool.capacities.is_empty() || tool.capacities.iter().any(|c| !nonempty(c)) {
            ctx.missing(
                &format!("/tools/{i}/capacities"),
                &format!("tool {n} capacity"),
            );
        }
        if tool.power.is_none() {
            ctx.missing(&format!("/tools/{i}/power"), &format!("tool {n} power"));
        }
        if tool.cooldown_time.is_none() {
            ctx.missing(
                &format!("/tools/{i}/cooldownTime"),
                &format!("tool {n} cooldown"),
            );
        }
    }
}

fn required_ranged(ctx: &mut Ctx<'_>) {
    let spec = ctx.spec;
    let empty = crate::model::RangedInputs::default();
    let r = spec.ranged.as_ref().unwrap_or(&empty);
    if r.damage.is_none() {
        ctx.missing("/ranged/damage", "damage");
    }
    if r.range.is_none() {
        ctx.missing("/ranged/range", "range");
    }
    if r.warmup.is_none() {
        ctx.missing("/ranged/warmup", "warmup");
    }
    if r.cooldown.is_none() {
        ctx.missing("/ranged/cooldown", "cooldown");
    }
    if r.accuracy.touch.is_none() {
        ctx.missing("/ranged/accuracy/touch", "touch accuracy");
    }
    if r.accuracy.short.is_none() {
        ctx.missing("/ranged/accuracy/short", "short accuracy");
    }
    if r.accuracy.medium.is_none() {
        ctx.missing("/ranged/accuracy/medium", "medium accuracy");
    }
    if r.accuracy.long.is_none() {
        ctx.missing("/ranged/accuracy/long", "long accuracy");
    }
    match &r.projectile {
        None => ctx.missing("/ranged/projectile", "projectile"),
        Some(ProjectileChoice::Reference(name)) if !nonempty(name) => {
            ctx.missing("/ranged/projectile/def", "projectile");
        }
        Some(ProjectileChoice::Inline(p)) => {
            if !nonempty(&p.def_name) {
                ctx.missing("/ranged/projectile/def/defName", "projectile def name");
            }
        }
        Some(ProjectileChoice::Reference(_)) => {}
    }
    if spec.cost_list.is_empty() {
        ctx.missing("/costList", "cost list");
    }
}

fn required_melee(ctx: &mut Ctx<'_>) {
    let spec = ctx.spec;
    if spec.tools.is_empty() {
        ctx.missing("/tools", "tools");
    }
    match &spec.stuff {
        Some(stuff) => {
            if stuff.categories.is_empty() {
                ctx.missing("/stuff/categories", "stuff categories");
            }
            if stuff.count.is_none() {
                ctx.missing("/stuff/count", "stuff count");
            }
        }
        None => {
            if spec.cost_list.is_empty() {
                ctx.missing("/stuff", "stuff categories and count, or a cost list");
            }
        }
    }
}

fn required_ce(ctx: &mut Ctx<'_>, ce: &CePatchSpec) {
    let spec = ctx.spec;
    if ce.bulk.is_none() {
        ctx.missing("/ce/bulk", "CE bulk");
    }
    match spec.kind {
        ItemKind::Ranged => {
            if !ce.ammo_set.as_deref().is_some_and(nonempty) {
                ctx.missing("/ce/ammoSet", "CE ammo set");
            }
            if !ce.default_projectile.as_deref().is_some_and(nonempty) {
                ctx.missing("/ce/defaultProjectile", "CE default projectile");
            }
            // A bow has no magazine: its spawn count and nothing else stands in for it.
            if !crate::ce::patchgen::is_bow_spec(spec) {
                if ce.magazine_size.is_none() {
                    ctx.missing("/ce/magazineSize", "CE magazine size");
                }
                if ce.reload_time.is_none() {
                    ctx.missing("/ce/reloadTime", "CE reload time");
                }
            }
            if ce.sway_factor.is_none() {
                ctx.missing("/ce/swayFactor", "CE sway factor");
            }
            if ce.shot_spread.is_none() {
                ctx.missing("/ce/shotSpread", "CE shot spread");
            }
        }
        ItemKind::Melee => {
            if ce.melee_crit_chance.is_none() {
                ctx.missing("/ce/meleeCritChance", "CE melee crit chance");
            }
            if ce.melee_parry_chance.is_none() {
                ctx.missing("/ce/meleeParryChance", "CE melee parry chance");
            }
            if ce.melee_dodge_chance.is_none() {
                ctx.missing("/ce/meleeDodgeChance", "CE melee dodge chance");
            }
            // An explicit tool plan names the penetration of every converted tool itself; the generator
            // checks the plan.
            for tool in spec.tools.iter().filter(|_| ce.tool_plan.is_empty()) {
                let needs_sharp = tool
                    .capacities
                    .iter()
                    .any(|c| matches!(c.as_str(), "Cut" | "Stab"));
                let found = ce
                    .tool_penetration
                    .iter()
                    .enumerate()
                    .find(|(_, p)| p.tool == tool.label);
                match found {
                    None => ctx.missing(
                        "/ce/toolPenetration",
                        &format!("CE penetration of tool {}", tool.label),
                    ),
                    Some((i, p)) => {
                        if p.blunt.is_none() {
                            ctx.missing(
                                &format!("/ce/toolPenetration/{i}/blunt"),
                                &format!("CE blunt penetration of tool {}", tool.label),
                            );
                        }
                        if needs_sharp && p.sharp.is_none() {
                            ctx.missing(
                                &format!("/ce/toolPenetration/{i}/sharp"),
                                &format!("CE sharp penetration of tool {}", tool.label),
                            );
                        }
                    }
                }
            }
        }
    }
    for (i, p) in ce.tool_penetration.iter().enumerate() {
        if !spec.tools.iter().any(|t| t.label == p.tool) {
            ctx.emit(
                codes::VALUE_INVALID,
                &format!("/ce/toolPenetration/{i}/tool"),
                &[
                    ("label", "CE tool penetration"),
                    ("value", &p.tool),
                    ("reason", "it names no tool of this item"),
                ],
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// structural value checks (IT-041)

fn values(ctx: &mut Ctx<'_>) {
    let want_ce = ctx.scope == Scope::Ce;
    let spec = ctx.spec;
    for (field, sourced) in spec.set_scalars() {
        if field.is_ce() != want_ce {
            continue;
        }
        let v = sourced.value;
        let label = field.label();
        let pointer = field.pointer();
        if !v.is_finite() {
            ctx.invalid_value(&pointer, &label, v, "must be a finite number");
            continue;
        }
        if v < 0.0 && !allows_negative(field) {
            ctx.invalid_value(&pointer, &label, v, "must not be negative");
        }
        if field.is_count() && v < 1.0 {
            ctx.invalid_value(&pointer, &label, v, "must be at least 1");
        }
        let game_reads_integer = matches!(
            field,
            ScalarField::StuffCount | ScalarField::TicksBetweenBurstShots
        );
        if game_reads_integer && !is_whole(v) {
            ctx.invalid_value(&pointer, &label, v, "the game reads a whole number here");
        }
    }
    if want_ce {
        return;
    }
    for (i, c) in spec.cost_list.iter().enumerate() {
        let pointer = format!("/costList/{i}/count");
        let label = format!("cost list entry {}", i + 1);
        if !c.count.is_finite() || c.count <= 0.0 {
            ctx.invalid_value(&pointer, &label, c.count, "must be greater than zero");
        } else if !is_whole(c.count) {
            ctx.invalid_value(
                &pointer,
                &label,
                c.count,
                "the game reads a whole number here",
            );
        }
    }
    for (name, stat) in &spec.extra_stats {
        if !stat.value.is_finite() {
            ctx.invalid_value(
                &format!("/extraStats/{}/value", escape_pointer(name)),
                name,
                stat.value,
                "must be a finite number",
            );
        }
    }
    if let Some(r) = &spec.ranged {
        if let Some(flash) = r.muzzle_flash_scale
            && (!flash.is_finite() || flash < 0.0)
        {
            ctx.invalid_value(
                "/ranged/muzzleFlashScale",
                "muzzle flash scale",
                flash,
                "must be a finite number that is not negative",
            );
        }
        if let Some(ProjectileChoice::Inline(p)) = &r.projectile {
            for (slot, label, tail) in [
                (p.speed, "projectile speed", "speed"),
                (
                    p.stopping_power,
                    "projectile stopping power",
                    "stoppingPower",
                ),
            ] {
                if let Some(s) = slot
                    && (!s.value.is_finite() || s.value < 0.0)
                {
                    ctx.invalid_value(
                        &format!("/ranged/projectile/def/{tail}"),
                        label,
                        s.value,
                        "must be a finite number that is not negative",
                    );
                }
            }
            if let Some(d) = val(r.damage)
                && !is_whole(d)
                && d.is_finite()
            {
                ctx.invalid_value(
                    "/ranged/damage",
                    "damage",
                    d,
                    "the projectile reads a whole number here",
                );
            }
        }
    }
}

fn escape_pointer(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

// ---------------------------------------------------------------------------------------------------------
// names

fn names_vanilla(ctx: &mut Ctx<'_>) {
    let spec = ctx.spec;
    ctx.check_token(
        "/identity/defName",
        "def name",
        &spec.identity.def_name,
        true,
    );
    if !spec.identity.mod_prefix.is_empty() {
        ctx.check_token(
            "/identity/modPrefix",
            "mod prefix",
            &spec.identity.mod_prefix,
            false,
        );
        check_prefix(ctx, "/identity/defName", &spec.identity.def_name);
    }
    if let Some(p) = &spec.parent {
        ctx.check_token("/parent/defName", "parent base", &p.def_name, false);
    }
    for (i, c) in spec.cost_list.iter().enumerate() {
        ctx.check_element_token(
            &format!("/costList/{i}/defName"),
            "cost list ingredient",
            &c.def_name,
        );
        if c.def_name.is_empty() {
            ctx.missing(
                &format!("/costList/{i}/defName"),
                &format!("cost list entry {} ingredient", i + 1),
            );
        }
    }
    if let Some(stuff) = &spec.stuff {
        for (i, c) in stuff.categories.iter().enumerate() {
            ctx.check_token(
                &format!("/stuff/categories/{i}"),
                "stuff category",
                c,
                false,
            );
        }
    }
    if let Some(r) = &spec.research_prerequisite {
        ctx.check_token("/researchPrerequisite", "research prerequisite", r, false);
    }
    for (list, pointer, label) in [
        (&spec.weapon_tags, "/weaponTags", "weapon tag"),
        (&spec.trade_tags, "/tradeTags", "trade tag"),
        (&spec.weapon_classes, "/weaponClasses", "weapon class"),
    ] {
        for (i, v) in list.iter().enumerate() {
            ctx.check_token(&format!("{pointer}/{i}"), label, v, false);
        }
    }
    for name in spec.extra_stats.keys() {
        ctx.check_element_token(
            &format!("/extraStats/{}", escape_pointer(name)),
            "stat name",
            name,
        );
    }
    for (i, tool) in spec.tools.iter().enumerate() {
        for (j, c) in tool.capacities.iter().enumerate() {
            ctx.check_token(&format!("/tools/{i}/capacities/{j}"), "capacity", c, false);
        }
        if let Some(g) = &tool.linked_body_parts_group {
            ctx.check_token(
                &format!("/tools/{i}/linkedBodyPartsGroup"),
                "body part group",
                g,
                false,
            );
        }
    }
    if let Some(r) = &spec.ranged {
        match &r.projectile {
            Some(ProjectileChoice::Reference(name)) => {
                ctx.check_token("/ranged/projectile/def", "projectile", name, false);
            }
            Some(ProjectileChoice::Inline(p)) => {
                ctx.check_token(
                    "/ranged/projectile/def/defName",
                    "projectile def name",
                    &p.def_name,
                    true,
                );
                if !spec.identity.mod_prefix.is_empty() {
                    check_prefix(ctx, "/ranged/projectile/def/defName", &p.def_name);
                }
                if let Some(parent) = &p.parent {
                    ctx.check_token(
                        "/ranged/projectile/def/parent",
                        "projectile parent",
                        parent,
                        false,
                    );
                }
                if let Some(d) = &p.damage_def {
                    ctx.check_token("/ranged/projectile/def/damageDef", "damage def", d, false);
                }
            }
            None => {}
        }
    }
}

fn check_prefix(ctx: &mut Ctx<'_>, field: &str, def_name: &str) {
    let prefix = &ctx.spec.identity.mod_prefix;
    if def_name.is_empty() || prefix.is_empty() {
        return;
    }
    let wanted = format!("{prefix}_");
    if !def_name.starts_with(&wanted) {
        let (def_name, prefix) = (def_name.to_owned(), prefix.clone());
        ctx.emit(
            codes::DEFNAME_PREFIX,
            field,
            &[("value", &def_name), ("prefix", &prefix)],
        );
    }
}

fn names_ce(ctx: &mut Ctx<'_>, ce: &CePatchSpec) {
    for (value, pointer, label) in [
        (&ce.ammo_set, "/ce/ammoSet", "CE ammo set"),
        (
            &ce.default_projectile,
            "/ce/defaultProjectile",
            "CE default projectile",
        ),
        (
            &ce.weapon_tag_class,
            "/ce/weaponTagClass",
            "CE weapon tag class",
        ),
        (&ce.caliber, "/ce/caliber", "CE caliber"),
    ] {
        if let Some(v) = value {
            ctx.check_token(pointer, label, v, false);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// free text and vanilla purity

fn strings(ctx: &mut Ctx<'_>) {
    let Ok(value) = serde_json::to_value(ctx.spec) else {
        return;
    };
    let mut found = Vec::new();
    collect_strings(&value, &mut String::new(), &mut found);
    for (pointer, text) in found {
        let in_ce = pointer == "/ce" || pointer.starts_with("/ce/");
        if in_ce != (ctx.scope == Scope::Ce) {
            continue;
        }
        if first_invalid_xml_char(&text).is_some() {
            ctx.emit(codes::TEXT_INVALID, &pointer, &[("label", &pointer)]);
        }
        if ctx.scope == Scope::Vanilla && text.contains(CE_CLASS_MARK) {
            ctx.emit(
                codes::CE_IN_VANILLA,
                &pointer,
                &[("label", &pointer), ("value", &text)],
            );
        }
    }
}

fn collect_strings(
    value: &serde_json::Value,
    pointer: &mut String,
    out: &mut Vec<(String, String)>,
) {
    match value {
        serde_json::Value::String(s) => out.push((pointer.clone(), s.clone())),
        serde_json::Value::Array(items) => {
            for (i, item) in items.iter().enumerate() {
                let len = pointer.len();
                pointer.push('/');
                pointer.push_str(&i.to_string());
                collect_strings(item, pointer, out);
                pointer.truncate(len);
            }
        }
        serde_json::Value::Object(map) => {
            for (k, item) in map {
                let len = pointer.len();
                pointer.push('/');
                pointer.push_str(&escape_pointer(k));
                collect_strings(item, pointer, out);
                pointer.truncate(len);
            }
        }
        _ => {}
    }
}

/// The first character XML 1.0 cannot represent, if any.
fn first_invalid_xml_char(text: &str) -> Option<char> {
    text.chars().find(|&c| {
        (c < ' ' && !matches!(c, '\t' | '\n' | '\r')) || c == '\u{FFFE}' || c == '\u{FFFF}'
    })
}

// ---------------------------------------------------------------------------------------------------------
// unit sanity (IT-031)

fn unit_flag(ctx: &mut Ctx<'_>, field: &str, label: &str, value: f64, reason: &str) {
    let text = crate::model::format_number(value);
    ctx.emit(
        codes::UNIT_MISMATCH,
        field,
        &[("label", label), ("value", &text), ("reason", reason)],
    );
}

fn units_vanilla(ctx: &mut Ctx<'_>) {
    let spec = ctx.spec;
    if let Some(m) = val(spec.mass)
        && m.is_finite()
        && m > MAX_PLAUSIBLE_MASS
    {
        unit_flag(
            ctx,
            "/mass",
            "mass",
            m,
            "is very heavy for a weapon; mass is in kilograms",
        );
    }
    if let Some(r) = spec
        .ranged
        .as_ref()
        .filter(|_| spec.kind == ItemKind::Ranged)
    {
        for (slot, field, label) in [
            (r.accuracy.touch, "/ranged/accuracy/touch", "touch accuracy"),
            (r.accuracy.short, "/ranged/accuracy/short", "short accuracy"),
            (
                r.accuracy.medium,
                "/ranged/accuracy/medium",
                "medium accuracy",
            ),
            (r.accuracy.long, "/ranged/accuracy/long", "long accuracy"),
        ] {
            if let Some(v) = val(slot)
                && v.is_finite()
                && v > 1.0
            {
                unit_flag(
                    ctx,
                    field,
                    label,
                    v,
                    "is above 1; accuracy is a fraction from 0 to 1, not a percent",
                );
            }
        }
        if let Some(ap) = val(r.armor_penetration)
            && ap.is_finite()
            && ap > MAX_RATING
        {
            unit_flag(
                ctx,
                "/ranged/armorPenetration",
                "armor penetration",
                ap,
                "is above the armor cap of 2; vanilla penetration is a fraction (mm of armor is a Combat Extended unit)",
            );
        }
        if let Some(range) = val(r.range)
            && range.is_finite()
            && range > MAX_PLAUSIBLE_RANGE
        {
            unit_flag(
                ctx,
                "/ranged/range",
                "range",
                range,
                "is very long; range is in cells",
            );
        }
        for (slot, field, label) in [
            (r.warmup, "/ranged/warmup", "warmup"),
            (r.cooldown, "/ranged/cooldown", "cooldown"),
        ] {
            if let Some(v) = val(slot)
                && v.is_finite()
                && v > MAX_PLAUSIBLE_SECONDS
            {
                unit_flag(
                    ctx,
                    field,
                    label,
                    v,
                    "is very long; this field is in seconds, not ticks",
                );
            }
        }
    }
    for (i, tool) in spec.tools.iter().enumerate() {
        if let Some(cd) = val(tool.cooldown_time)
            && cd.is_finite()
            && cd > MAX_PLAUSIBLE_SECONDS
        {
            unit_flag(
                ctx,
                &format!("/tools/{i}/cooldownTime"),
                &format!("tool {} cooldown", i + 1),
                cd,
                "is very long; this field is in seconds, not ticks",
            );
        }
        if let Some(ap) = val(tool.armor_penetration)
            && ap.is_finite()
            && ap > MAX_RATING
        {
            unit_flag(
                ctx,
                &format!("/tools/{i}/armorPenetration"),
                &format!("tool {} armor penetration", i + 1),
                ap,
                "is above the armor cap of 2; vanilla penetration is a fraction",
            );
        }
    }
}

fn units_ce(ctx: &mut Ctx<'_>) {
    let Some(ce) = ctx.spec.ce.as_ref() else {
        return;
    };
    if let Some(t) = val(ce.reload_time)
        && t.is_finite()
        && t > MAX_PLAUSIBLE_SECONDS
    {
        unit_flag(
            ctx,
            "/ce/reloadTime",
            "CE reload time",
            t,
            "is very long; this field is in seconds, not ticks",
        );
    }
}

// ---------------------------------------------------------------------------------------------------------
// tools (IT-035) and list hygiene

fn tools(ctx: &mut Ctx<'_>) {
    let spec = ctx.spec;
    for (i, tool) in spec.tools.iter().enumerate() {
        if tool.capacities.len() >= 2 {
            let count = tool.capacities.len().to_string();
            ctx.emit(
                codes::DUPLICATE_CAPACITY,
                &format!("/tools/{i}/capacities"),
                &[("tool", &tool.label), ("count", &count)],
            );
        }
    }
}

fn lists(ctx: &mut Ctx<'_>) {
    let spec = ctx.spec;
    let mut seen = BTreeSet::new();
    for (i, c) in spec.cost_list.iter().enumerate() {
        if !c.def_name.is_empty() && !seen.insert(c.def_name.as_str()) {
            ctx.emit(
                codes::DUPLICATE_ENTRY,
                &format!("/costList/{i}"),
                &[("label", "cost list"), ("value", &c.def_name)],
            );
        }
    }
    let mut lists: Vec<(&Vec<String>, &str, &str)> = vec![
        (&spec.weapon_tags, "/weaponTags", "weapon tags"),
        (&spec.trade_tags, "/tradeTags", "trade tags"),
        (&spec.weapon_classes, "/weaponClasses", "weapon classes"),
    ];
    if let Some(stuff) = &spec.stuff {
        lists.push((&stuff.categories, "/stuff/categories", "stuff categories"));
    }
    for (list, pointer, label) in lists {
        let mut seen = BTreeSet::new();
        for (i, v) in list.iter().enumerate() {
            if !v.is_empty() && !seen.insert(v.as_str()) {
                ctx.emit(
                    codes::DUPLICATE_ENTRY,
                    &format!("{pointer}/{i}"),
                    &[("label", label), ("value", v)],
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// vanilla definition rules

fn vanilla_rules(ctx: &mut Ctx<'_>) {
    let spec = ctx.spec;
    let def_name = spec.identity.def_name.as_str();
    if let Some(p) = &spec.parent
        && !def_name.is_empty()
        && p.def_name == def_name
    {
        ctx.emit(
            codes::VALUE_INVALID,
            "/parent/defName",
            &[
                ("label", "parent base"),
                ("value", def_name),
                ("reason", "a def cannot be its own parent"),
            ],
        );
    }
    if let Some(ProjectileChoice::Inline(p)) =
        spec.ranged.as_ref().and_then(|r| r.projectile.as_ref())
        && !def_name.is_empty()
        && p.def_name == def_name
    {
        ctx.emit(
            codes::DEFNAME_CONFLICT,
            "/ranged/projectile/def/defName",
            &[("value", def_name), ("other", "the weapon")],
        );
    }
    if spec.kind == ItemKind::Melee && spec.ranged.is_some() {
        ctx.emit(
            codes::IGNORED_INPUT,
            "/ranged",
            &[("label", "the ranged inputs"), ("kind", spec.kind.as_str())],
        );
    }
    if let Some(mv) = val(spec.market_value) {
        let text = crate::model::format_number(mv);
        ctx.emit(codes::EXPLICIT_PRICE, "/marketValue", &[("value", &text)]);
    }
}

// ---------------------------------------------------------------------------------------------------------
// carried fields

/// Checks one list of carried raw nodes: each must be a valid element tree and must not repeat a field the
/// designer writes itself (`known`).
fn check_extras(ctx: &mut Ctx<'_>, pointer: &str, place: &str, nodes: &[Node], known: &[&str]) {
    for (i, node) in nodes.iter().enumerate() {
        let field = format!("{pointer}/{i}");
        if known.contains(&node.tag.as_str()) {
            ctx.emit(
                codes::EXTRA_FIELD_CONFLICT,
                &field,
                &[("name", &node.tag), ("place", place)],
            );
        }
        if let Err(e) = node.validate() {
            ctx.emit(
                codes::EXTRA_FIELD_INVALID,
                &field,
                &[
                    ("name", &node.tag),
                    ("place", place),
                    ("reason", &e.to_string()),
                ],
            );
        }
    }
}

/// Checks nodes that are written as list entries: they must be `li` elements.
fn check_list_entries(ctx: &mut Ctx<'_>, pointer: &str, place: &str, nodes: &[Node]) {
    check_extras(ctx, pointer, place, nodes, &[]);
    for (i, node) in nodes.iter().enumerate() {
        if node.tag != "li" {
            ctx.emit(
                codes::EXTRA_FIELD_INVALID,
                &format!("{pointer}/{i}"),
                &[
                    ("name", &node.tag),
                    ("place", place),
                    ("reason", "a list entry must be an li element"),
                ],
            );
        }
    }
}

fn carried(ctx: &mut Ctx<'_>) {
    let spec = ctx.spec;
    check_extras(
        ctx,
        "/extraFields",
        "the definition",
        &spec.extra_fields,
        &MODELLED_THING_FIELDS,
    );
    check_list_entries(ctx, "/comps", "comps", &spec.comps);
    check_list_entries(ctx, "/otherVerbs", "verbs", &spec.other_verbs);
    check_extras(
        ctx,
        "/graphicExtra",
        "graphicData",
        &spec.graphic_extra,
        &["texPath", "graphicClass", "drawSize", "color"],
    );
    if let Some(recipe) = &spec.recipe {
        check_extras(
            ctx,
            "/recipe/extra",
            "recipeMaker",
            &recipe.extra,
            &[
                "researchPrerequisite",
                "skillRequirements",
                "displayPriority",
                "recipeUsers",
                "unfinishedThingDef",
                "workSkill",
            ],
        );
        for skill in recipe.skill_requirements.keys() {
            ctx.check_element_token(
                &format!("/recipe/skillRequirements/{}", escape_pointer(skill)),
                "skill",
                skill,
            );
        }
        if let Some(p) = recipe.display_priority
            && !p.is_finite()
        {
            ctx.invalid_value(
                "/recipe/displayPriority",
                "display priority",
                p,
                "must be a finite number",
            );
        }
    }
    for (name, value) in &spec.equipped_stat_offsets {
        let pointer = format!("/equippedStatOffsets/{}", escape_pointer(name));
        ctx.check_element_token(&pointer, "stat name", name);
        if !value.is_finite() {
            ctx.invalid_value(&pointer, name, *value, "must be a finite number");
        }
    }
    if let Some(scale) = spec.ui_icon_scale
        && (!scale.is_finite() || scale <= 0.0)
    {
        ctx.invalid_value(
            "/uiIconScale",
            "icon scale",
            scale,
            "must be a finite number above zero",
        );
    }
    for (i, tool) in spec.tools.iter().enumerate() {
        check_extras(
            ctx,
            &format!("/tools/{i}/extra"),
            "a tool",
            &tool.extra,
            &[
                "label",
                "capacities",
                "power",
                "cooldownTime",
                "armorPenetration",
                "chanceFactor",
                "linkedBodyPartsGroup",
                "extraMeleeDamages",
                "surpriseAttack",
            ],
        );
    }
    if let Some(r) = &spec.ranged {
        check_extras(
            ctx,
            "/ranged/verbExtra",
            "the shooting verb",
            &r.verb_extra,
            &[
                "verbClass",
                "defaultProjectile",
                "range",
                "burstShotCount",
                "ticksBetweenBurstShots",
                "warmupTime",
                "soundCast",
                "soundCastTail",
                "muzzleFlashScale",
                "hasStandardCommand",
                "forcedMissRadius",
            ],
        );
        if let Some(radius) = r.forced_miss_radius
            && (!radius.is_finite() || radius < 0.0)
        {
            ctx.invalid_value(
                "/ranged/forcedMissRadius",
                "forced miss radius",
                radius,
                "must be a finite number that is not negative",
            );
        }
        if let Some(ProjectileChoice::Inline(p)) = &r.projectile {
            check_extras(
                ctx,
                "/ranged/projectile/def/extra",
                "the projectile element",
                &p.extra,
                &[
                    "damageDef",
                    "damageAmountBase",
                    "stoppingPower",
                    "armorPenetrationBase",
                    "speed",
                ],
            );
            check_extras(
                ctx,
                "/ranged/projectile/def/graphicExtra",
                "the projectile graphicData",
                &p.graphic_extra,
                &["texPath", "graphicClass"],
            );
            check_extras(
                ctx,
                "/ranged/projectile/def/thingExtra",
                "the projectile definition",
                &p.thing_extra,
                &["defName", "label", "graphicData", "projectile"],
            );
        }
    }
    for (i, name) in spec.inherit_reset.iter().enumerate() {
        if !INHERIT_RESETTABLE.contains(&name.as_str()) {
            ctx.emit(
                codes::VALUE_INVALID,
                &format!("/inheritReset/{i}"),
                &[
                    ("label", "inherit reset"),
                    ("value", name),
                    (
                        "reason",
                        "this list cannot be written with Inherit=\"False\"",
                    ),
                ],
            );
        }
    }
}

// ---------------------------------------------------------------------------------------------------------
// references against loaded defs

/// The kinds of def a reference can point at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum RefKind {
    /// A thing def (parent bases, ingredients, projectiles).
    Thing,
    /// A research project def.
    ResearchProject,
    /// A body part group def.
    BodyPartGroup,
    /// A tool capacity def.
    ToolCapacity,
    /// A damage def.
    DamageDef,
    /// A stuff category def.
    StuffCategory,
    /// A weapon class def.
    WeaponClass,
    /// A sound def.
    SoundDef,
    /// A skill def.
    SkillDef,
}

/// Answers whether a def exists among the loaded defs and the project. The toolkit implements it over the
/// def databases; a draft opened without an install cannot, which is the `None` answer.
pub trait DefLookup {
    /// `Some(true)` when the def exists, `Some(false)` when it does not, `None` when the answer is not
    /// known (no install, no data of that kind). Unknown answers produce no diagnostic.
    fn contains(&self, kind: RefKind, name: &str) -> Option<bool>;
}

/// Checks the def name against the loaded defs (`design.defname-conflict`) and every reference of the
/// vanilla definition (`design.ref-unresolved`). Unknown answers are skipped. CE references are not
/// checked here.
#[must_use]
pub fn validate_refs(spec: &DesignSpec, lookup: &dyn DefLookup) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut unresolved = |kind: RefKind, pointer: String, label: &str, name: &str| {
        if !name.is_empty() && lookup.contains(kind, name) == Some(false) {
            out.push(
                codes::REF_UNRESOLVED.diagnostic(&pointer, &[("label", label), ("value", name)]),
            );
        }
    };
    if let Some(p) = &spec.parent {
        unresolved(
            RefKind::Thing,
            "/parent/defName".into(),
            "parent base",
            &p.def_name,
        );
    }
    for (i, c) in spec.cost_list.iter().enumerate() {
        unresolved(
            RefKind::Thing,
            format!("/costList/{i}/defName"),
            "cost list ingredient",
            &c.def_name,
        );
    }
    if let Some(stuff) = &spec.stuff {
        for (i, c) in stuff.categories.iter().enumerate() {
            unresolved(
                RefKind::StuffCategory,
                format!("/stuff/categories/{i}"),
                "stuff category",
                c,
            );
        }
    }
    if let Some(r) = &spec.research_prerequisite {
        unresolved(
            RefKind::ResearchProject,
            "/researchPrerequisite".into(),
            "research prerequisite",
            r,
        );
    }
    for (i, c) in spec.weapon_classes.iter().enumerate() {
        unresolved(
            RefKind::WeaponClass,
            format!("/weaponClasses/{i}"),
            "weapon class",
            c,
        );
    }
    for (i, tool) in spec.tools.iter().enumerate() {
        for (j, c) in tool.capacities.iter().enumerate() {
            unresolved(
                RefKind::ToolCapacity,
                format!("/tools/{i}/capacities/{j}"),
                "capacity",
                c,
            );
        }
        if let Some(g) = &tool.linked_body_parts_group {
            unresolved(
                RefKind::BodyPartGroup,
                format!("/tools/{i}/linkedBodyPartsGroup"),
                "body part group",
                g,
            );
        }
    }
    if let Some(sound) = &spec.sound_interact {
        unresolved(
            RefKind::SoundDef,
            "/soundInteract".into(),
            "interaction sound",
            sound,
        );
    }
    if let Some(recipe) = &spec.recipe {
        for skill in recipe.skill_requirements.keys() {
            unresolved(
                RefKind::SkillDef,
                format!("/recipe/skillRequirements/{}", escape_pointer(skill)),
                "skill",
                skill,
            );
        }
        for (i, user) in recipe.recipe_users.iter().enumerate() {
            unresolved(
                RefKind::Thing,
                format!("/recipe/recipeUsers/{i}"),
                "workbench",
                user,
            );
        }
        if let Some(t) = &recipe.unfinished_thing_def {
            unresolved(
                RefKind::Thing,
                "/recipe/unfinishedThingDef".into(),
                "unfinished thing",
                t,
            );
        }
        if let Some(w) = &recipe.work_skill {
            unresolved(
                RefKind::SkillDef,
                "/recipe/workSkill".into(),
                "work skill",
                w,
            );
        }
    }
    if let Some(r) = &spec.ranged {
        for (pointer, label, slot) in [
            ("/ranged/soundCast", "shot sound", &r.sound_cast),
            (
                "/ranged/soundCastTail",
                "shot tail sound",
                &r.sound_cast_tail,
            ),
        ] {
            // a custom shot sound replaces whatever `soundCast` held, so the old name is not checked
            if pointer == "/ranged/soundCast" && spec.sounds.shot.is_some() {
                continue;
            }
            if let Some(sound) = slot {
                unresolved(RefKind::SoundDef, pointer.into(), label, sound);
            }
        }
        match &r.projectile {
            Some(ProjectileChoice::Reference(name)) => {
                unresolved(
                    RefKind::Thing,
                    "/ranged/projectile/def".into(),
                    "projectile",
                    name,
                );
            }
            Some(ProjectileChoice::Inline(p)) => {
                if let Some(parent) = &p.parent {
                    unresolved(
                        RefKind::Thing,
                        "/ranged/projectile/def/parent".into(),
                        "projectile parent",
                        parent,
                    );
                }
                if let Some(d) = &p.damage_def {
                    unresolved(
                        RefKind::DamageDef,
                        "/ranged/projectile/def/damageDef".into(),
                        "damage def",
                        d,
                    );
                }
            }
            None => {}
        }
    }
    let mut conflicts = |pointer: &str, name: &str, other: &str| {
        if !name.is_empty() && lookup.contains(RefKind::Thing, name) == Some(true) {
            out.push(
                codes::DEFNAME_CONFLICT.diagnostic(pointer, &[("value", name), ("other", other)]),
            );
        }
    };
    conflicts("/identity/defName", &spec.identity.def_name, "a loaded def");
    if let Some(ProjectileChoice::Inline(p)) =
        spec.ranged.as_ref().and_then(|r| r.projectile.as_ref())
    {
        conflicts(
            "/ranged/projectile/def/defName",
            &p.def_name,
            "a loaded def",
        );
    }
    if let Some(name) = crate::model::shot_sound_def_name(spec)
        && lookup.contains(RefKind::SoundDef, &name) == Some(true)
    {
        out.push(super::asset_codes::SOUND_DUPLICATE.diagnostic(
            "/sounds/shot/defName",
            &[("value", &name), ("other", "a loaded def")],
        ));
    }
    out
}
