//! The registry of the lint rules CEP001 to CEP022 and of the other `ce.*` codes of the patch generator.
//!
//! A rule id is embedded in its code as `ce.cep<nnn>-<kebab-name>`, which keeps the `<area>.<kebab-name>`
//! form and the stable id together. CEP006 is not defined in the research script and stays unassigned.
//! Severities follow the specification: error when the patch silently never applies or breaks loading,
//! warning when the effect is log noise or a missing nicety.

use rimstudio_core::diag::Severity;

use crate::validation::CodeInfo;

macro_rules! rule {
    ($(#[$doc:meta])* $ident:ident, $code:literal, $sev:ident, $tpl:literal, [$($arg:literal),*]) => {
        $(#[$doc])*
        pub const $ident: CodeInfo = CodeInfo {
            code: $code,
            severity: Severity::$sev,
            requirement: "CP-015",
            template: $tpl,
            args: &[$($arg),*],
        };
    };
}

rule!(
    /// CEP001: a `FindMod` entry looks like a package id.
    CEP001, "ce.cep001-findmod-looks-like-packageid", Error,
    "the FindMod entry {value} looks like a package id; FindMod compares the mod name, so it never matches", ["value"]
);
rule!(
    /// CEP002: a `FindMod` entry spells Combat Extended differently from the exact name.
    CEP002, "ce.cep002-findmod-name-spelling", Error,
    "the FindMod entry {value} does not spell the mod name exactly as {name}", ["value", "name"]
);
rule!(
    /// CEP003: a `FindMod` entry has leading or trailing white space.
    CEP003, "ce.cep003-findmod-whitespace", Error,
    "the FindMod entry {value} has leading or trailing white space; names are compared without trimming", ["value"]
);
rule!(
    /// CEP004: a Combat Extended class sits in a file that loads without Combat Extended.
    CEP004, "ce.cep004-ce-class-ungated", Error,
    "the operation uses the Combat Extended class {class} in a file that loads when Combat Extended is absent", ["class"]
);
rule!(
    /// CEP005: `MayRequire` on an operation, which the game ignores there.
    CEP005, "ce.cep005-mayrequire-on-operation", Warning,
    "MayRequire on {element} is ignored; it is honoured only on list items", ["element"]
);
rule!(
    /// CEP007: the same def is converted more than once.
    CEP007, "ce.cep007-makegun-repeated", Error,
    "the def {value} is converted more than once; comps, verbs and tags are appended again", ["value"]
);
rule!(
    /// CEP008: a gun conversion misses a section or key field.
    CEP008, "ce.cep008-makegun-incomplete", Error,
    "the gun conversion of {def} lacks {missing}", ["def", "missing"]
);
rule!(
    /// CEP009: the verb class is not a Combat Extended verb class.
    CEP009, "ce.cep009-verb-class-not-ce", Warning,
    "the verb class {value} is not a Combat Extended verb class", ["value"]
);
rule!(
    /// CEP010: a class attribute names a Combat Extended type that does not exist.
    CEP010, "ce.cep010-unknown-ce-type", Error,
    "the class {value} is not a type of the installed Combat Extended", ["value"]
);
rule!(
    /// CEP011: a converted tool without sharp and blunt penetration.
    CEP011, "ce.cep011-toolce-no-penetration", Warning,
    "the tool {tool} has the Combat Extended class but no penetration; every armor blocks it", ["tool"]
);
rule!(
    /// CEP012: replaced tools without the Combat Extended class.
    CEP012, "ce.cep012-tool-without-ce-class", Warning,
    "the tool {tool} has no Combat Extended class and loads as a plain tool without penetration", ["tool"]
);
rule!(
    /// CEP013: the ammo set is not defined.
    CEP013, "ce.cep013-ammoset-unresolved", Error,
    "the ammo set {value} is not defined in Combat Extended, the project or the patched mod", ["value"]
);
rule!(
    /// CEP014: the default projectile is not defined.
    CEP014, "ce.cep014-projectile-unresolved", Error,
    "the projectile {value} is not defined in Combat Extended, vanilla, the project or the patched mod", ["value"]
);
rule!(
    /// CEP015: a field name is not known to its section.
    CEP015, "ce.cep015-unknown-field", Warning,
    "the field {value} is not a known field of {section}", ["value", "section"]
);
rule!(
    /// CEP016: a Combat Extended weapon tag is unknown to the installed Combat Extended.
    CEP016, "ce.cep016-unknown-ce-tag", Warning,
    "the weapon tag {value} is not known to the installed Combat Extended", ["value"]
);
rule!(
    /// CEP017: the file is not a patch file.
    CEP017, "ce.cep017-patch-file-shape", Error,
    "the patch file is not valid: {reason}", ["reason"]
);
rule!(
    /// CEP018: the file sits in a folder that `LoadFolders.xml` never loads.
    CEP018, "ce.cep018-folder-not-loaded", Error,
    "the folder {folder} is not loaded by LoadFolders.xml for version {version}; the patch never applies", ["folder", "version"]
);
rule!(
    /// CEP019: an `IfModActive` id that cannot match.
    CEP019, "ce.cep019-ifmodactive-id-variant", Error,
    "the id {value} cannot match: the game ignores a suffix of the active mod's id, not of the written one", ["value"]
);
rule!(
    /// CEP020: a folder or extension in a different case.
    CEP020, "ce.cep020-case-mismatch", Warning,
    "the path {value} differs in case from {expected}; this breaks on case sensitive file systems", ["value", "expected"]
);
rule!(
    /// CEP021: an exact duplicate operation.
    CEP021, "ce.cep021-duplicate-operation", Warning,
    "this operation repeats an earlier one exactly ({class} on {xpath})", ["class", "xpath"]
);
rule!(
    /// CEP022: a malformed xpath.
    CEP022, "ce.cep022-xpath-malformed", Error,
    "the xpath {value} is malformed: {reason}", ["value", "reason"]
);
rule!(
    /// A data dependent rule could not run because Combat Extended data is not available.
    NOT_CHECKED, "ce.not-checked", Info,
    "{rule} was not checked: {reason}", ["rule", "reason"]
);
rule!(
    /// Update mode writes into a RimStudio file that must load after the original conversion.
    UPDATE_LOAD_AFTER, "ce.update-load-after", Hint,
    "the update patch of {target} must run after the original conversion ({source}); add loadAfter for Combat Extended to the About file", ["target", "source"]
);
rule!(
    /// A suggestion for the About file.
    ABOUT_SUGGESTION, "ce.about-suggestion", Hint,
    "About file: {suggestion}", ["suggestion"]
);
rule!(
    /// A number of the patch was derived instead of given.
    DERIVED_VALUE, "ce.derived-value", Info,
    "{field} = {value} was {how}", ["field", "value", "how"]
);
rule!(
    /// A predicted number is far from the vanilla number it replaces.
    DERIVED_FAR, "ce.derived-far-from-vanilla", Warning,
    "{field} = {value} was predicted, but the vanilla definition has {vanilla}; give the number yourself if the prediction is wrong", ["field", "value", "vanilla"]
);
rule!(
    /// Nothing differs between the design and the existing conversion.
    UPDATE_NOTHING, "ce.update-nothing", Info,
    "{target} already holds these values; no operation was written", ["target"]
);
rule!(
    /// A feature is not available in this release.
    DEFERRED, "design.deferred", Info,
    "{what} is not available in this release", ["what"]
);
rule!(
    /// A weapon tag the user asked for does not exist in the installed Combat Extended.
    TAG_NOT_FOUND, "ce.tag-not-found", Hint,
    "no Combat Extended tag matching {what} was found in the installed data", ["what"]
);

/// Every rule of this module, in id order.
pub const REGISTRY: &[CodeInfo] = &[
    CEP001, CEP002, CEP003, CEP004, CEP005, CEP007, CEP008, CEP009, CEP010, CEP011, CEP012, CEP013,
    CEP014, CEP015, CEP016, CEP017, CEP018, CEP019, CEP020, CEP021, CEP022,
];

/// The other codes of the patch generator.
pub const OTHER: &[CodeInfo] = &[
    NOT_CHECKED,
    UPDATE_LOAD_AFTER,
    ABOUT_SUGGESTION,
    DERIVED_VALUE,
    DERIVED_FAR,
    UPDATE_NOTHING,
    DEFERRED,
    TAG_NOT_FOUND,
];

/// The rule id (`CEP013`) of a rule code (`ce.cep013-ammoset-unresolved`), when it is one.
#[must_use]
pub fn rule_id(code: &str) -> Option<String> {
    let rest = code.strip_prefix("ce.cep")?;
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    (digits.len() == 3).then(|| format!("CEP{digits}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_codes_embed_their_ids_and_are_unique() {
        assert_eq!(REGISTRY.len(), 21);
        let mut seen = std::collections::BTreeSet::new();
        for (i, info) in REGISTRY.iter().enumerate() {
            let id = rule_id(info.code).unwrap_or_default();
            assert!(id.starts_with("CEP"), "{}", info.code);
            assert!(seen.insert(id.clone()), "{id}");
            if i == 0 {
                assert_eq!(id, "CEP001");
            }
            assert_ne!(id, "CEP006");
            assert!(info.diag_code().is_well_formed(), "{}", info.code);
        }
        for info in OTHER {
            assert!(info.diag_code().is_well_formed(), "{}", info.code);
        }
    }

    #[test]
    fn rule_id_ignores_other_codes() {
        assert_eq!(
            rule_id("ce.cep013-ammoset-unresolved").as_deref(),
            Some("CEP013")
        );
        assert_eq!(rule_id("ce.not-checked"), None);
    }
}
