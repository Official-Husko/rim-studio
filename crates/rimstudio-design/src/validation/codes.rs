//! The registry of designer diagnostic codes (section 7 of the items toolkit specification).
//!
//! Every code has a stable name (`<area>.<kebab-name>`), a default severity, a message template with named
//! arguments (`{name}`) and the requirement it serves. The UI translates by code and arguments; the template
//! is the English fallback. Codes are API: a rename needs a release note.
//!
//! Codes that need data the pure checks do not have (peer pools, bands, an install, the CE data) are
//! registered here so that every producer shares one table; the pure checks of this crate emit only the
//! subset marked "pure" in their documentation. The Combat Extended lint rules (`ce.cep001` and the rest)
//! are registered by the CE lint module.

use rimstudio_core::diag::{DiagCode, Diagnostic, Severity};

/// The argument key under which a diagnostic carries the JSON pointer of the field it concerns.
///
/// The pointer is relative to the `spec` object of a draft (`/ranged/damage`, `/tools/1/power`); the empty
/// pointer means the whole spec. A UI prefixes `/spec` for a draft document.
pub const FIELD_ARG: &str = "field";

/// One registered diagnostic code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CodeInfo {
    /// The stable code, `<area>.<kebab-name>`.
    pub code: &'static str,
    /// The default severity.
    pub severity: Severity,
    /// The requirement id this code serves (for example `IT-030`).
    pub requirement: &'static str,
    /// English message template with `{name}` placeholders.
    pub template: &'static str,
    /// The argument names the template uses (the field pointer is always added on top).
    pub args: &'static [&'static str],
}

impl CodeInfo {
    /// The code as a [`DiagCode`].
    #[must_use]
    pub fn diag_code(&self) -> DiagCode {
        DiagCode::new(self.code)
    }

    /// Builds a diagnostic for this code. `field` is the JSON pointer of the field it concerns; `args` fill
    /// the message template (extra arguments are kept for the UI).
    #[must_use]
    pub fn diagnostic(&self, field: &str, args: &[(&str, &str)]) -> Diagnostic {
        let mut d = Diagnostic::new(
            self.diag_code(),
            self.severity,
            render_template(self.template, args),
        );
        for (k, v) in args {
            d = d.with_arg(*k, *v);
        }
        d.with_arg(FIELD_ARG, field)
    }
}

/// Fills `{name}` placeholders from `args`. Unknown placeholders stay as written.
#[must_use]
pub fn render_template(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) => {
                let name = &after[..close];
                match args.iter().find(|(k, _)| *k == name) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The field pointer a diagnostic carries, if any.
#[must_use]
pub fn diagnostic_field(diagnostic: &Diagnostic) -> Option<&str> {
    diagnostic.args.get(FIELD_ARG).map(String::as_str)
}

/// True when any diagnostic is an error. Only errors block writing (IT-041).
#[must_use]
pub fn has_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.severity == Severity::Error)
}

macro_rules! code {
    ($(#[$doc:meta])* $ident:ident, $code:literal, $sev:ident, $req:literal, $tpl:literal, [$($arg:literal),*]) => {
        $(#[$doc])*
        pub const $ident: CodeInfo = CodeInfo {
            code: $code,
            severity: Severity::$sev,
            requirement: $req,
            template: $tpl,
            args: &[$($arg),*],
        };
    };
}

code!(
    /// A required field of the item kind is empty (pure).
    REQUIRED_MISSING, "design.required-missing", Error, "IT-030",
    "{label} is required", ["label"]
);
code!(
    /// A reference does not resolve in the loaded defs (needs an install; see `validate_refs`).
    REF_UNRESOLVED, "design.ref-unresolved", Error, "IT-040",
    "{label} refers to {value}, which was not found", ["label", "value"]
);
code!(
    /// The def name already exists, or two defs of the draft share one (pure for the draft part).
    DEFNAME_CONFLICT, "design.defname-conflict", Error, "IT-040",
    "def name {value} is already used by {other}", ["value", "other"]
);
code!(
    /// A value is outside the P50 or P80 band of its prediction (needs the baseline).
    STAT_OUT_OF_BAND, "design.stat-out-of-band", Warning, "IT-042",
    "{label} {value} is outside the {band} band of the prediction", ["label", "value", "band"]
);
code!(
    /// A value is outside the observed range of its pool (needs the pool).
    STAT_OUTSIDE_PEERS, "design.stat-outside-peers", Warning, "IT-042",
    "{label} {value} is outside the range of its peers ({min} to {max})", ["label", "value", "min", "max"]
);
code!(
    /// The computed price is far from the model price (needs the baseline).
    PRICE_FAR_FROM_MODEL, "design.price-far-from-model", Warning, "IT-033",
    "the computed price {value} differs from the model price {model} by more than a factor of 2", ["value", "model"]
);
code!(
    /// An armor rating exceeds the game cap (apparel; registered for the shared table).
    RATING_OVER_CAP, "design.rating-over-cap", Warning, "IT-040",
    "{label} {value} exceeds the armor cap of {cap}", ["label", "value", "cap"]
);
code!(
    /// A stack of thin layers underperforms one thick layer (apparel; registered for the shared table).
    LAYER_STACK_WEAK, "design.layer-stack-weak", Info, "IT-040",
    "the layer stack is expected to underperform one thick layer against penetration {value}", ["value"]
);
code!(
    /// Fixed armor above 0.9 sharp with no move speed offset (apparel; registered for the shared table).
    SPEED_OFFSET_MISSING, "design.speed-offset-missing", Warning, "IT-040",
    "the armor rating {value} has no move speed offset", ["value"]
);
code!(
    /// Range rises sharply with no longer warmup (needs the baseline).
    RANGE_WARMUP_JUMP, "design.range-warmup-jump", Warning, "IT-040",
    "range {range} is high for a warmup of {warmup}", ["range", "warmup"]
);
code!(
    /// Bulk and mass disagree with the class (needs the baseline).
    MASS_BULK_MISMATCH, "design.mass-bulk-mismatch", Info, "IT-040",
    "bulk {bulk} and mass {mass} disagree with the class", ["bulk", "mass"]
);
code!(
    /// CE melee penetration is out of line with the class (needs CE data).
    MELEE_PEN_LOW, "design.melee-pen-low", Warning, "IT-040",
    "penetration {value} of tool {tool} is out of line with its class", ["value", "tool"]
);
code!(
    /// A melee tool lists two capacities, so the stat panel counts its attack twice (pure).
    DUPLICATE_CAPACITY, "design.duplicate-capacity", Warning, "IT-035",
    "tool {tool} lists {count} capacities; the stat panel counts its attack once per capacity", ["tool", "count"]
);
code!(
    /// The standard handle attack lowers in-fight DPS by more than 5 percent (needs the melee math).
    HANDLE_DRAGS_DPS, "design.handle-drags-dps", Info, "IT-035",
    "the attack of tool {tool} lowers in-fight DPS by {percent} percent", ["tool", "percent"]
);
code!(
    /// An explicit market value overrides the formula (pure).
    EXPLICIT_PRICE, "design.explicit-price", Info, "IT-033",
    "the explicit market value {value} overrides the value computed from cost list and work", ["value"]
);
code!(
    /// Fewer than 15 reference items stand behind the bands (needs the pool).
    POOL_THIN, "design.pool-thin", Info, "IT-040",
    "only {count} reference items stand behind the bands", ["count"]
);
code!(
    /// The calibration cache key changed (needs the cache).
    CALIBRATION_STALE, "design.calibration-stale", Info, "IT-040",
    "the calibration is out of date: {reason}", ["reason"]
);
code!(
    /// A value is on the wrong scale or in the wrong unit (pure).
    UNIT_MISMATCH, "design.unit-mismatch", Warning, "IT-031",
    "{label} {value} {reason}", ["label", "value", "reason"]
);
code!(
    /// The CE patch toggle is disabled because no CE data is available (needs the install).
    CE_ABSENT, "design.ce-absent", Info, "IT-005",
    "the Combat Extended patch is unavailable: no Combat Extended found", []
);
code!(
    /// The target already carries a CE conversion; update mode is used (CE patch generator).
    CE_ALREADY_CONVERTED, "ce.already-converted", Info, "IT-057",
    "{target} already carries a Combat Extended conversion; update mode is used", ["target"]
);
code!(
    /// A generated operation fails in the app's own patch engine (CE patch generator).
    CE_DRY_RUN_FAILED, "ce.dry-run-failed", Error, "IT-056",
    "operation {operation} fails in the patch engine: {reason}", ["operation", "reason"]
);
code!(
    /// A def name or reference is not a valid def name (pure).
    NAME_INVALID, "design.name-invalid", Error, "IT-041",
    "{label} {value} is not a valid name: {reason}", ["label", "value", "reason"]
);
code!(
    /// The def name does not start with the mod prefix (pure).
    DEFNAME_PREFIX, "design.defname-prefix", Warning, "IT-040",
    "def name {value} does not start with the mod prefix {prefix}_", ["value", "prefix"]
);
code!(
    /// A value is structurally impossible: not finite, negative, not a whole number where the game reads one
    /// (pure).
    VALUE_INVALID, "design.value-invalid", Error, "IT-041",
    "{label} {value} is not valid: {reason}", ["label", "value", "reason"]
);
code!(
    /// Free text holds a character XML cannot represent (pure).
    TEXT_INVALID, "design.text-invalid", Error, "IT-041",
    "{label} holds a character that cannot be written to XML", ["label"]
);
code!(
    /// The draft envelope and its spec disagree about the item kind (pure).
    DRAFT_INCONSISTENT, "design.draft-inconsistent", Error, "IT-041",
    "the draft kind {draft} differs from the spec kind {spec}", ["draft", "spec"]
);
code!(
    /// A vanilla field names a Combat Extended class; vanilla definitions never hold CE classes (pure).
    CE_IN_VANILLA, "design.ce-in-vanilla", Error, "IT-052",
    "{label} {value} names a Combat Extended class; the vanilla definition never holds one", ["label", "value"]
);
code!(
    /// An input is ignored for this kind of item (pure).
    IGNORED_INPUT, "design.ignored-input", Info, "IT-040",
    "{label} is ignored for a {kind} item", ["label", "kind"]
);
code!(
    /// The same entry appears twice in a list (pure).
    DUPLICATE_ENTRY, "design.duplicate-entry", Warning, "IT-040",
    "{label} lists {value} more than once", ["label", "value"]
);
code!(
    /// A planned path is not a safe relative path (plan builder).
    PLAN_PATH_INVALID, "design.plan-path-invalid", Error, "IT-004",
    "the path {path} is not a safe relative path", ["path"]
);
code!(
    /// Two planned files share one path (plan builder).
    PLAN_PATH_CONFLICT, "design.plan-path-conflict", Error, "IT-004",
    "two planned files use the path {path}", ["path"]
);
code!(
    /// The weapon has no texture path of its own, so the plan reserves the conventional one (plan builder).
    TEXTURE_RESERVED, "design.texture-reserved", Info, "IT-050",
    "the art of {label} goes to {path}; the file does not exist yet", ["label", "path"]
);

/// Every registered code, in a stable order.
pub const REGISTRY: &[CodeInfo] = &[
    REQUIRED_MISSING,
    REF_UNRESOLVED,
    DEFNAME_CONFLICT,
    STAT_OUT_OF_BAND,
    STAT_OUTSIDE_PEERS,
    PRICE_FAR_FROM_MODEL,
    RATING_OVER_CAP,
    LAYER_STACK_WEAK,
    SPEED_OFFSET_MISSING,
    RANGE_WARMUP_JUMP,
    MASS_BULK_MISMATCH,
    MELEE_PEN_LOW,
    DUPLICATE_CAPACITY,
    HANDLE_DRAGS_DPS,
    EXPLICIT_PRICE,
    POOL_THIN,
    CALIBRATION_STALE,
    UNIT_MISMATCH,
    CE_ABSENT,
    CE_ALREADY_CONVERTED,
    CE_DRY_RUN_FAILED,
    NAME_INVALID,
    DEFNAME_PREFIX,
    VALUE_INVALID,
    TEXT_INVALID,
    DRAFT_INCONSISTENT,
    CE_IN_VANILLA,
    IGNORED_INPUT,
    DUPLICATE_ENTRY,
    PLAN_PATH_INVALID,
    PLAN_PATH_CONFLICT,
    TEXTURE_RESERVED,
];

/// Looks a code up in the registry.
#[must_use]
pub fn lookup(code: &str) -> Option<&'static CodeInfo> {
    REGISTRY.iter().find(|c| c.code == code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    /// The codes of the table in section 7 that this registry must hold (IT-040).
    const SPEC_CODES: &[&str] = &[
        "design.required-missing",
        "design.ref-unresolved",
        "design.defname-conflict",
        "design.stat-out-of-band",
        "design.stat-outside-peers",
        "design.price-far-from-model",
        "design.rating-over-cap",
        "design.layer-stack-weak",
        "design.speed-offset-missing",
        "design.range-warmup-jump",
        "design.mass-bulk-mismatch",
        "design.melee-pen-low",
        "design.duplicate-capacity",
        "design.handle-drags-dps",
        "design.explicit-price",
        "design.pool-thin",
        "design.calibration-stale",
        "design.unit-mismatch",
        "design.ce-absent",
        "ce.already-converted",
        "ce.dry-run-failed",
    ];

    #[test]
    fn registry_holds_every_code_of_the_specification() {
        for code in SPEC_CODES {
            assert!(lookup(code).is_some(), "{code} is not registered");
        }
    }

    #[test]
    fn codes_are_well_formed_unique_and_in_a_known_area() {
        let mut seen = std::collections::BTreeSet::new();
        for info in REGISTRY {
            assert!(info.diag_code().is_well_formed(), "{}", info.code);
            assert!(seen.insert(info.code), "duplicate {}", info.code);
            assert!(matches!(info.diag_code().area(), "design" | "ce"));
        }
    }

    #[test]
    fn default_severities_match_the_specification() {
        assert_eq!(REQUIRED_MISSING.severity, Severity::Error);
        assert_eq!(REF_UNRESOLVED.severity, Severity::Error);
        assert_eq!(DEFNAME_CONFLICT.severity, Severity::Error);
        assert_eq!(DUPLICATE_CAPACITY.severity, Severity::Warning);
        assert_eq!(EXPLICIT_PRICE.severity, Severity::Info);
        assert_eq!(UNIT_MISMATCH.severity, Severity::Warning);
        assert_eq!(CE_ABSENT.severity, Severity::Info);
        assert_eq!(CE_DRY_RUN_FAILED.severity, Severity::Error);
    }

    #[test]
    fn templates_use_only_declared_arguments() {
        for info in REGISTRY {
            let mut rest = info.template;
            while let Some(open) = rest.find('{') {
                let after = &rest[open + 1..];
                let close = after.find('}').unwrap();
                let name = &after[..close];
                assert!(
                    info.args.contains(&name),
                    "{}: undeclared {name}",
                    info.code
                );
                rest = &after[close + 1..];
            }
            for arg in info.args {
                assert!(
                    info.template.contains(&format!("{{{arg}}}")),
                    "{}: unused {arg}",
                    info.code
                );
            }
        }
    }

    #[rstest]
    #[case("{a} and {b}", &[("a", "1"), ("b", "2")], "1 and 2")]
    #[case("{a} {missing}", &[("a", "x")], "x {missing}")]
    #[case("no placeholders", &[], "no placeholders")]
    #[case("open {brace", &[], "open {brace")]
    fn template_rendering(#[case] t: &str, #[case] args: &[(&str, &str)], #[case] want: &str) {
        assert_eq!(render_template(t, args), want);
    }

    #[test]
    fn diagnostics_carry_the_field_pointer_and_arguments() {
        let d = REQUIRED_MISSING.diagnostic("/ranged/damage", &[("label", "damage")]);
        assert_eq!(d.code.as_str(), "design.required-missing");
        assert_eq!(d.message, "damage is required");
        assert_eq!(diagnostic_field(&d), Some("/ranged/damage"));
        assert!(has_errors(&[d]));
        assert!(!has_errors(&[
            EXPLICIT_PRICE.diagnostic("/marketValue", &[("value", "5")])
        ]));
    }
}
