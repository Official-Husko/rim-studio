//! Pure edits of draft JSON: new drafts, `key=value` settings, strength fill and the opt in switch of
//! the Combat Extended block.
//!
//! A draft travels as the JSON of `DraftDto` (`{"kind", "calibration", "spec": {...}}`). Everything here
//! works on that JSON so the CLI never needs the design engine, and nothing here touches a file.

use rimstudio_ipc_types::designer::{
    DesignSpecDto, DraftDto, ItemKindDto, RangedInputsDto, ToolSpecDto,
};
use serde_json::{Map, Value, json};

use crate::cli::{KindArg, StrengthArg};
use crate::fmt::label_of;

/// Reads text as JSON when it is JSON (numbers, booleans, null, arrays, objects, quoted strings) and as
/// plain text otherwise.
#[must_use]
pub(crate) fn parse_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_owned()))
}

/// A fresh draft of a kind with its names; every other member is absent (vanilla only, no Combat Extended
/// block).
///
/// # Errors
/// A message when the draft cannot be encoded.
pub(crate) fn new_draft(
    kind: KindArg,
    def_name: &str,
    label: Option<&str>,
) -> Result<Value, String> {
    let item = match kind {
        KindArg::Ranged => ItemKindDto::Ranged,
        KindArg::Melee => ItemKindDto::Melee,
    };
    let mut spec = DesignSpecDto::new(item);
    spec.identity.def_name = def_name.to_owned();
    spec.identity.label = label.map_or_else(|| label_of(def_name), str::to_owned);
    match kind {
        KindArg::Ranged => spec.ranged = Some(RangedInputsDto::default()),
        KindArg::Melee => {
            spec.tools = vec![ToolSpecDto {
                label: "head".to_owned(),
                capacities: vec!["Cut".to_owned()],
                ..ToolSpecDto::default()
            }];
        }
    }
    serde_json::to_value(DraftDto::new(spec))
        .map_err(|e| format!("the draft cannot be encoded: {e}"))
}

const ALIASES: [(&str, &[&str]); 5] = [
    ("name", &["identity", "defName"]),
    ("label", &["identity", "label"]),
    ("description", &["identity", "description"]),
    ("prefix", &["identity", "modPrefix"]),
    ("tier", &["techLevel"]),
];

fn segments(key: &str) -> Vec<String> {
    let key = key.strip_prefix("spec.").unwrap_or(key);
    for (alias, path) in ALIASES {
        if key == alias {
            return path.iter().map(|s| (*s).to_owned()).collect();
        }
    }
    key.split('.').map(str::to_owned).collect()
}

fn is_list_field(segs: &[String]) -> bool {
    let last = segs.last().map_or("", String::as_str);
    matches!(
        last,
        "weaponTags"
            | "tradeTags"
            | "weaponClasses"
            | "categories"
            | "capacities"
            | "thingCategories"
            | "users"
            | "alternatives"
            | "researchPrerequisites"
    )
}

fn is_plain_number(segs: &[String]) -> bool {
    let last = segs.last().map_or("", String::as_str);
    last == "muzzleFlashScale"
        || segs.first().is_some_and(|s| s == "costList")
        || (last == "count" && segs.iter().any(|s| s == "fragments"))
}

fn is_text_field(segs: &[String]) -> bool {
    let last = segs.last().map_or("", String::as_str);
    matches!(
        last,
        "defName"
            | "label"
            | "description"
            | "modPrefix"
            | "role"
            | "techLevel"
            | "previewQuality"
            | "researchPrerequisite"
            | "texturePath"
            | "graphicClass"
            | "verbClass"
            | "soundCast"
            | "soundCastTail"
            | "caliber"
            | "ammoSet"
            | "defaultProjectile"
            | "weaponTagClass"
            | "linkedBodyPartsGroup"
            | "name"
            | "key"
            | "ammoClass"
            | "setLabel"
            | "similarTo"
            | "categoryParent"
            | "categoryIcon"
            | "defaultType"
            | "parent"
            | "thingClass"
            | "damageDef"
            | "texPath"
            | "drawSize"
            | "casingMote"
            | "casingFilth"
            | "soundExplode"
            | "soundAmbient"
            | "soundHitThickRoof"
            | "soundImpactAnticipate"
            | "thing"
            | "jobString"
            | "copiedFrom"
    )
}

fn typed_number(n: &Value) -> Value {
    json!({"value": n, "source": "typed"})
}

fn comma_list(raw: &str) -> Value {
    Value::Array(
        raw.split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| Value::String(s.to_owned()))
            .collect(),
    )
}

fn cost_list(raw: &str) -> Result<Value, String> {
    let mut out = Vec::new();
    for part in raw.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let (name, count) = part.rsplit_once(':').unwrap_or((part, "1"));
        let count: f64 = count
            .trim()
            .parse()
            .map_err(|_| format!("`{part}` is not NAME:COUNT"))?;
        out.push(json!({"defName": name.trim(), "count": count}));
    }
    Ok(Value::Array(out))
}

/// The value written for a key, after the special forms of the key are applied.
fn leaf_for(segs: &[String], raw: &str) -> Result<Value, String> {
    let joined = segs.join(".");
    let parsed = parse_value(raw);
    match joined.as_str() {
        "costList" if !parsed.is_array() => return cost_list(raw),
        "parent" => {
            return Ok(match parsed {
                Value::String(name) => json!({"defName": name}),
                other => other,
            });
        }
        "ranged.projectile" => {
            return Ok(match parsed {
                Value::String(name) => json!({"mode": "reference", "def": name}),
                Value::Object(map) if !map.contains_key("mode") => {
                    json!({"mode": "inline", "def": map})
                }
                other => other,
            });
        }
        "techLevel" | "previewQuality" => {
            if let Value::String(text) = &parsed {
                return Ok(Value::String(text.to_lowercase()));
            }
        }
        _ => {}
    }
    if is_list_field(segs) && !parsed.is_array() {
        return Ok(comma_list(raw));
    }
    if is_text_field(segs) {
        return Ok(Value::String(raw.to_owned()));
    }
    match parsed {
        Value::Number(_) if is_plain_number(segs) => Ok(parsed),
        Value::Number(_) => Ok(typed_number(&parsed)),
        Value::Bool(_) | Value::Object(_) | Value::Array(_) | Value::Null => Ok(parsed),
        _ => Ok(Value::String(raw.to_owned())),
    }
}

fn set_at(node: &mut Value, segs: &[String], leaf: Value) -> Result<(), String> {
    let Some((head, rest)) = segs.split_first() else {
        *node = leaf;
        return Ok(());
    };
    let index = head.parse::<usize>().ok();
    if node.is_null() {
        *node = if index.is_some() {
            Value::Array(Vec::new())
        } else {
            Value::Object(Map::new())
        };
    }
    match node {
        Value::Array(items) => {
            let i = index.ok_or_else(|| format!("`{head}` is not a list position"))?;
            if i > items.len() {
                return Err(format!(
                    "position {i} is past the end of the list (it has {} entries)",
                    items.len()
                ));
            }
            if i == items.len() {
                items.push(Value::Null);
            }
            match items.get_mut(i) {
                Some(slot) => set_at(slot, rest, leaf),
                None => Err(format!("position {i} does not exist")),
            }
        }
        Value::Object(map) => {
            let slot = map.entry(head.clone()).or_insert(Value::Null);
            set_at(slot, rest, leaf)
        }
        _ => Err(format!("`{head}` cannot be set inside a plain value")),
    }
}

/// Sets one value of the spec of a draft.
///
/// `key` is a dotted path into the spec (`ranged.damage`, `tools.0.power`, `ce.ammoSet`); the aliases
/// `name`, `label`, `description`, `prefix` and `tier` are accepted. A number becomes a typed value
/// (`{"value": n, "source": "typed"}`) except for the few plain numbers; `weaponTags=a,b` splits a list;
/// `costList=NAME:COUNT,...` builds the recipe; `ranged.projectile=NAME` references a projectile.
///
/// # Errors
/// A message naming the problem.
pub(crate) fn set_value(draft: &mut Value, key: &str, raw: &str) -> Result<(), String> {
    let segs = segments(key);
    if segs.iter().any(String::is_empty) {
        return Err(format!("`{key}` is not a field path"));
    }
    let leaf = leaf_for(&segs, raw)?;
    let spec = draft
        .get_mut("spec")
        .ok_or_else(|| "the draft has no spec".to_owned())?;
    set_at(spec, &segs, leaf)
}

/// Splits `key=value`.
///
/// # Errors
/// A message when there is no `=` or the key is empty.
pub(crate) fn split_assignment(text: &str) -> Result<(&str, &str), String> {
    match text.split_once('=') {
        Some((k, v)) if !k.trim().is_empty() => Ok((k.trim(), v)),
        _ => Err(format!("`{text}` is not KEY=VALUE")),
    }
}

/// Applies several `key=value` settings in order.
///
/// # Errors
/// A message naming the first setting that fails.
pub(crate) fn apply_sets(draft: &mut Value, sets: &[String]) -> Result<(), String> {
    for item in sets {
        let (k, v) = split_assignment(item)?;
        set_value(draft, k, v).map_err(|e| format!("--set {item}: {e}"))?;
    }
    Ok(())
}

/// Removes the Combat Extended block: the designer writes vanilla only unless asked.
pub(crate) fn strip_ce(draft: &mut Value) {
    if let Some(spec) = draft.get_mut("spec").and_then(Value::as_object_mut) {
        spec.remove("ce");
    }
}

/// Makes sure the Combat Extended block exists (the opt in of `--ce`).
pub(crate) fn ensure_ce(draft: &mut Value) {
    if let Some(spec) = draft.get_mut("spec").and_then(Value::as_object_mut) {
        let slot = spec.entry("ce").or_insert(Value::Null);
        if !slot.is_object() {
            *slot = json!({});
        }
    }
}

/// True when the draft carries a Combat Extended block.
#[must_use]
pub(crate) fn has_ce(draft: &Value) -> bool {
    draft.pointer("/spec/ce").is_some_and(Value::is_object)
}

fn pointer_segments(pointer: &str) -> Vec<String> {
    pointer
        .split('/')
        .skip(1)
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect()
}

fn present(draft: &Value, segs: &[String]) -> bool {
    let mut node = draft.get("spec");
    for seg in segs {
        node = match node {
            Some(Value::Array(items)) => seg.parse::<usize>().ok().and_then(|i| items.get(i)),
            Some(Value::Object(map)) => map.get(seg),
            _ => None,
        };
    }
    node.is_some_and(|v| !v.is_null())
}

const LOWER_IS_STRONGER: [&str; 3] = ["cooldown", "warmup", "ticks_between"];
const HIGHER_IS_STRONGER: [&str; 8] = [
    "damage",
    "range",
    "burst",
    "touch",
    "short",
    "medium",
    "long",
    "swing_damage",
];
const FRACTIONS: [&str; 4] = ["touch", "short", "medium", "long"];

/// A count of at least one as a JSON integer (burst counts are whole numbers).
fn whole(v: f64) -> Value {
    let n = v.round().max(1.0);
    // the value is finite and at least one; a burst count never comes near the range limit
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let int = if n.is_finite() {
        n.min(1_000_000.0) as u64
    } else {
        1
    };
    json!(int)
}

fn shaded(stat: &str, value: f64, band: Option<&Value>, strength: StrengthArg) -> f64 {
    let lower_better = LOWER_IS_STRONGER.contains(&stat);
    let higher_better = HIGHER_IS_STRONGER.contains(&stat);
    if strength == StrengthArg::Typical || !(lower_better || higher_better) {
        return value;
    }
    let toward_strong = (strength == StrengthArg::Stronger) == higher_better;
    let edge = band
        .and_then(|b| b.get(if toward_strong { "p90" } else { "p10" }))
        .and_then(Value::as_f64);
    let target = edge.unwrap_or(if toward_strong {
        value * 1.15
    } else {
        value * 0.85
    });
    value + (target - value) * 0.5
}

/// Fills the numbers that are still absent from the suggestions of a preview, shaded by `strength`.
///
/// Only fields that are absent are written, a locked suggestion is skipped, and every value is marked
/// `suggested`, so a number the user typed is never replaced. Returns how many fields were filled.
pub(crate) fn fill_from_suggestions(
    draft: &mut Value,
    suggestions: &[Value],
    strength: StrengthArg,
) -> usize {
    let mut filled = 0;
    for s in suggestions {
        let (Some(field), Some(value)) = (
            s.get("field").and_then(Value::as_str),
            s.get("value").and_then(Value::as_f64),
        ) else {
            continue;
        };
        if s.get("locked").and_then(Value::as_bool).unwrap_or(false) {
            continue;
        }
        let segs = pointer_segments(field);
        if segs.is_empty() || present(draft, &segs) {
            continue;
        }
        let stat = s.get("stat").and_then(Value::as_str).unwrap_or("");
        let mut v = shaded(stat, value, s.get("band"), strength);
        if FRACTIONS.contains(&stat) {
            v = v.clamp(0.0, 1.0);
        }
        let number = if stat == "burst" {
            whole(v)
        } else {
            json!((v * 10_000.0).round() / 10_000.0)
        };
        let leaf = json!({"value": number, "source": "suggested"});
        let ok = draft
            .get_mut("spec")
            .is_some_and(|spec| set_at(spec, &segs, leaf).is_ok());
        if ok {
            filled += 1;
        }
    }
    filled
}

/// Copies the numbers of a reference weapon (an item of `designer_reference_list`) into a draft, marked
/// `anchor`: tech level, role and the pool stats the item carries. Existing numbers are kept.
pub(crate) fn apply_reference(draft: &mut Value, item: &Value) -> usize {
    let mut filled = 0;
    let kind = draft
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("ranged")
        .to_owned();
    if let Some(tier) = item.get("tier").and_then(Value::as_str)
        && let Some(spec) = draft.get_mut("spec").and_then(Value::as_object_mut)
        && !spec.contains_key("techLevel")
    {
        spec.insert("techLevel".into(), json!(tier));
        filled += 1;
    }
    if let Some(role) = item.get("role").and_then(Value::as_str)
        && let Some(spec) = draft.get_mut("spec").and_then(Value::as_object_mut)
        && !spec.contains_key("role")
    {
        spec.insert("role".into(), json!(role));
        filled += 1;
    }
    let stats = item
        .get("stats")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let melee = kind == "melee";
    let map: &[(&str, &str)] = if melee {
        &[
            ("mass", "mass"),
            ("work", "workToMake"),
            ("swing_damage", "tools.0.power"),
            ("cooldown", "tools.0.cooldownTime"),
        ]
    } else {
        &[
            ("mass", "mass"),
            ("work", "workToMake"),
            ("damage", "ranged.damage"),
            ("warmup", "ranged.warmup"),
            ("cooldown", "ranged.cooldown"),
            ("range", "ranged.range"),
            ("burst", "ranged.burstCount"),
            ("ticks_between", "ranged.ticksBetweenBurstShots"),
            ("touch", "ranged.accuracy.touch"),
            ("short", "ranged.accuracy.short"),
            ("medium", "ranged.accuracy.medium"),
            ("long", "ranged.accuracy.long"),
        ]
    };
    for (stat, path) in map {
        let Some(v) = stats.get(*stat).and_then(Value::as_f64) else {
            continue;
        };
        let segs = segments(path);
        if present(draft, &segs) {
            continue;
        }
        let number = if *stat == "burst" {
            whole(v)
        } else {
            // Pool statistics carry long fractions; a written definition should read like a hand written one.
            json!((v * 10_000.0).round() / 10_000.0)
        };
        let leaf = json!({"value": number, "source": "anchor"});
        if draft
            .get_mut("spec")
            .is_some_and(|spec| set_at(spec, &segs, leaf).is_ok())
        {
            filled += 1;
        }
    }
    filled
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn ranged() -> Value {
        new_draft(KindArg::Ranged, "RS_TestRifle", None).unwrap_or(Value::Null)
    }

    #[test]
    fn a_new_ranged_draft_is_vanilla_with_derived_names() {
        let d = ranged();
        assert_eq!(d["kind"], json!("ranged"));
        assert_eq!(d["spec"]["identity"]["defName"], json!("RS_TestRifle"));
        assert_eq!(d["spec"]["identity"]["label"], json!("rs test rifle"));
        assert!(!has_ce(&d));
        assert!(d["spec"]["ranged"].is_object());
    }

    #[test]
    fn a_new_melee_draft_has_one_tool() {
        let d = new_draft(KindArg::Melee, "RS_TestBlade", Some("blade")).unwrap_or(Value::Null);
        assert_eq!(d["spec"]["tools"][0]["label"], json!("head"));
        assert_eq!(d["spec"]["identity"]["label"], json!("blade"));
        assert!(d["spec"].get("ranged").is_none());
    }

    #[rstest]
    #[case("ranged.damage", "12", json!({"value": 12, "source": "typed"}))]
    #[case("mass", "3.5", json!({"value": 3.5, "source": "typed"}))]
    #[case("ranged.accuracy.touch", "0.7", json!({"value": 0.7, "source": "typed"}))]
    #[case("ranged.muzzleFlashScale", "0.5", json!(0.5))]
    #[case("tier", "Industrial", json!("industrial"))]
    #[case("role", "RS_Rifle", json!("RS_Rifle"))]
    #[case("ranged.soundCast", "RS_Sound", json!("RS_Sound"))]
    #[case("parent", "RS_BaseGun", json!({"defName": "RS_BaseGun"}))]
    #[case("weaponTags", "RS_A, RS_B", json!(["RS_A", "RS_B"]))]
    #[case("weaponTags", "[\"RS_X\"]", json!(["RS_X"]))]
    #[case("ranged.projectile", "RS_Shot00", json!({"mode": "reference", "def": "RS_Shot00"}))]
    #[case("ranged.projectile", "{\"defName\":\"RS_B\"}", json!({"mode": "inline", "def": {"defName": "RS_B"}}))]
    #[case("costList", "RS_Steel:30,RS_Part:2", json!([{"defName": "RS_Steel", "count": 30.0}, {"defName": "RS_Part", "count": 2.0}]))]
    #[case("ce.oneHanded", "true", json!(true))]
    #[case("ce.magazineSize", "30", json!({"value": 30, "source": "typed"}))]
    #[case("name", "RS_Other", json!("RS_Other"))]
    fn values_are_written_in_the_shape_of_the_spec(
        #[case] key: &str,
        #[case] raw: &str,
        #[case] expected: Value,
    ) {
        let mut d = ranged();
        set_value(&mut d, key, raw).unwrap_or_default();
        let segs = segments(key);
        let mut node = &d["spec"];
        for s in &segs {
            node = &node[s.as_str()];
        }
        assert_eq!(node, &expected, "{key}");
    }

    #[test]
    fn a_text_field_keeps_digits_as_text() {
        let mut d = ranged();
        set_value(&mut d, "role", "123").unwrap_or_default();
        assert_eq!(d["spec"]["role"], json!("123"));
    }

    #[test]
    fn tool_fields_create_list_entries_in_order() {
        let mut d = new_draft(KindArg::Melee, "RS_B", None).unwrap_or(Value::Null);
        set_value(&mut d, "tools.0.power", "9").unwrap_or_default();
        set_value(&mut d, "tools.1.label", "handle").unwrap_or_default();
        assert_eq!(d["spec"]["tools"][0]["power"]["value"], json!(9));
        assert_eq!(d["spec"]["tools"][1]["label"], json!("handle"));
        assert!(set_value(&mut d, "tools.5.label", "x").is_err());
    }

    #[test]
    fn bad_keys_and_assignments_are_refused() {
        let mut d = ranged();
        assert!(set_value(&mut d, "a..b", "1").is_err());
        assert!(set_value(&mut d, "identity.defName.deeper", "1").is_err());
        assert!(split_assignment("novalue").is_err());
        assert!(split_assignment("=x").is_err());
        assert_eq!(split_assignment("a=b=c").ok(), Some(("a", "b=c")));
        assert!(apply_sets(&mut d, &["ranged.damage".to_owned()]).is_err());
    }

    #[test]
    fn the_combat_extended_block_is_added_and_removed_explicitly() {
        let mut d = ranged();
        assert!(!has_ce(&d));
        ensure_ce(&mut d);
        assert!(has_ce(&d));
        set_value(&mut d, "ce.ammoSet", "RS_AmmoSetA").unwrap_or_default();
        assert_eq!(d["spec"]["ce"]["ammoSet"], json!("RS_AmmoSetA"));
        strip_ce(&mut d);
        assert!(!has_ce(&d));
        assert!(d["spec"].get("ce").is_none());
    }

    fn suggestion(field: &str, stat: &str, value: f64, locked: bool) -> Value {
        json!({"field": field, "stat": stat, "value": value, "locked": locked,
               "band": {"p10": value * 0.5, "median": value, "p90": value * 2.0}})
    }

    #[test]
    fn suggestions_fill_only_absent_unlocked_fields_and_never_replace_typed_values() {
        let mut d = ranged();
        set_value(&mut d, "ranged.damage", "20").unwrap_or_default();
        let s = vec![
            suggestion("/ranged/damage", "damage", 10.0, false),
            suggestion("/ranged/range", "range", 25.0, false),
            suggestion("/ranged/warmup", "warmup", 1.0, true),
        ];
        let n = fill_from_suggestions(&mut d, &s, StrengthArg::Typical);
        assert_eq!(n, 1);
        assert_eq!(d["spec"]["ranged"]["damage"]["value"], json!(20));
        assert_eq!(
            d["spec"]["ranged"]["range"],
            json!({"value": 25.0, "source": "suggested"})
        );
        assert!(d["spec"]["ranged"].get("warmup").is_none());
    }

    #[rstest]
    #[case(StrengthArg::Weaker, "damage", 7.5)]
    #[case(StrengthArg::Typical, "damage", 10.0)]
    #[case(StrengthArg::Stronger, "damage", 15.0)]
    #[case(StrengthArg::Weaker, "cooldown", 15.0)]
    #[case(StrengthArg::Stronger, "cooldown", 7.5)]
    #[case(StrengthArg::Stronger, "mass", 10.0)]
    fn strength_shades_toward_the_pool_edge_in_the_right_direction(
        #[case] strength: StrengthArg,
        #[case] stat: &str,
        #[case] expected: f64,
    ) {
        let band = json!({"p10": 5.0, "median": 10.0, "p90": 20.0});
        assert!((shaded(stat, 10.0, Some(&band), strength) - expected).abs() < 1e-9);
    }

    #[test]
    fn accuracy_stays_between_zero_and_one_and_burst_is_a_whole_number() {
        let mut d = ranged();
        let s = vec![
            json!({"field": "/ranged/accuracy/touch", "stat": "touch", "value": 0.95, "locked": false, "band": {"p10": 0.5, "median": 0.95, "p90": 1.4}}),
            json!({"field": "/ranged/burstCount", "stat": "burst", "value": 2.4, "locked": false}),
        ];
        fill_from_suggestions(&mut d, &s, StrengthArg::Stronger);
        let touch = d["spec"]["ranged"]["accuracy"]["touch"]["value"]
            .as_f64()
            .unwrap_or(9.0);
        assert!(touch <= 1.0, "{touch}");
        assert_eq!(d["spec"]["ranged"]["burstCount"]["value"], json!(3));
    }

    #[test]
    fn a_reference_fills_tier_role_and_stats_as_anchor_values() {
        let mut d = ranged();
        set_value(&mut d, "ranged.damage", "99").unwrap_or_default();
        let item = json!({"tier": "industrial", "role": "RS_Rifle",
            "stats": {"damage": 12.0, "range": 30.0, "mass": 3.0, "short": 0.8, "burst": 1.0}});
        let n = apply_reference(&mut d, &item);
        assert_eq!(n, 6);
        assert_eq!(d["spec"]["techLevel"], json!("industrial"));
        assert_eq!(d["spec"]["ranged"]["damage"]["value"], json!(99));
        assert_eq!(d["spec"]["ranged"]["range"]["source"], json!("anchor"));
        assert_eq!(
            d["spec"]["ranged"]["accuracy"]["short"]["value"],
            json!(0.8)
        );
    }

    #[test]
    fn reference_numbers_are_rounded_to_four_decimals() {
        let mut d = ranged();
        let item = json!({"stats": {"range": 25.912345678, "mass": 3.0}});
        apply_reference(&mut d, &item);
        assert_eq!(d["spec"]["ranged"]["range"]["value"], json!(25.9123));
        assert_eq!(d["spec"]["mass"]["value"], json!(3.0));
    }

    #[test]
    fn parse_value_reads_json_or_text() {
        assert_eq!(parse_value("12"), json!(12));
        assert_eq!(parse_value("true"), json!(true));
        assert_eq!(parse_value("hello world"), json!("hello world"));
        assert_eq!(parse_value("\"q\""), json!("q"));
    }
}
