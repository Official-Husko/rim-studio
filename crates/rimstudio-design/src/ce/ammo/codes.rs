//! Diagnostic codes of the custom ammunition: the checks of the spec (`ce.ammo-*`) and the lint rules for the
//! generated definitions (`ce.cep050` and following).

use rimstudio_core::diag::Severity;

use crate::validation::CodeInfo;

macro_rules! code {
    ($(#[$doc:meta])* $ident:ident, $code:literal, $sev:ident, $tpl:literal, [$($arg:literal),*]) => {
        $(#[$doc])*
        pub const $ident: CodeInfo = CodeInfo {
            code: $code,
            severity: Severity::$sev,
            requirement: "CP-030",
            template: $tpl,
            args: &[$($arg),*],
        };
    };
}

code!(
    /// The custom ammunition has no name.
    NAME_MISSING, "ce.ammo-name-missing", Error,
    "the custom ammunition needs a name; the def names derive from it", []
);
code!(
    /// The name is not usable in a def name.
    NAME_INVALID, "ce.ammo-name-invalid", Error,
    "the name {name} of the custom ammunition cannot be used in a def name: {reason}", ["name", "reason"]
);
code!(
    /// The custom ammunition has no caliber label.
    CALIBER_MISSING, "ce.ammo-caliber-missing", Error,
    "the custom ammunition needs a caliber label", []
);
code!(
    /// The custom ammunition has no type.
    NO_TYPES, "ce.ammo-no-types", Error,
    "the custom ammunition needs at least one ammo type", []
);
code!(
    /// A type has no ammo class.
    CLASS_MISSING, "ce.ammo-class-missing", Error,
    "the ammo type {type} needs an ammo class", ["type"]
);
code!(
    /// The ammo class of a type is not an ammo class of the install.
    CLASS_UNKNOWN, "ce.ammo-class-unknown", Error,
    "the ammo class {class} of the ammo type {type} is not defined by the installed Combat Extended", ["type", "class"]
);
code!(
    /// Two types have the same key.
    KEY_DUPLICATE, "ce.ammo-key-duplicate", Error,
    "two ammo types have the key {key}; the def names of the types must differ", ["key"]
);
code!(
    /// A required field of a type is missing.
    FIELD_REQUIRED, "ce.ammo-field-required", Error,
    "the ammo type {type} needs {what}", ["type", "what"]
);
code!(
    /// A number is not usable.
    NUMBER_INVALID, "ce.ammo-number-invalid", Error,
    "{what} of the ammo type {type} is {value}, which is not usable: {reason}", ["type", "what", "value", "reason"]
);
code!(
    /// A number lies far outside what the user's own ammunition of the class has.
    IMPLAUSIBLE, "ce.ammo-implausible", Warning,
    "{what} of the ammo type {type} is {value}; the {n} installed ammo types of the class {class} range from {low} to {high}", ["type", "what", "value", "n", "class", "low", "high"]
);
code!(
    /// A derived def name already exists.
    DUPLICATE_DEF, "ce.ammo-duplicate-def", Error,
    "the def name {name} already exists; change the name of the custom ammunition", ["name"]
);
code!(
    /// A reference does not resolve.
    REF_UNRESOLVED, "ce.ammo-ref-unresolved", Error,
    "{what} {value} is not defined", ["what", "value"]
);
code!(
    /// A reference could not be checked or is doubtful.
    REF_DOUBTFUL, "ce.ammo-ref-doubtful", Warning,
    "{what} {value} was not found among the installed ammunition data", ["what", "value"]
);
code!(
    /// A text cannot be written as XML.
    TEXT_INVALID, "ce.ammo-text-invalid", Error,
    "{what} holds text that cannot be written: {reason}", ["what", "reason"]
);
code!(
    /// The default type is not a type of the custom ammunition.
    DEFAULT_UNKNOWN, "ce.ammo-default-unknown", Error,
    "the default type {key} is not one of the ammo types", ["key"]
);
code!(
    /// An item or projectile has no parent.
    NO_PARENT, "ce.ammo-no-parent", Warning,
    "the {what} of the ammo type {type} has no parent def; it will lack what the installed Combat Extended gives its own ammunition", ["type", "what"]
);
code!(
    /// A recipe has no ingredient.
    RECIPE_EMPTY, "ce.ammo-recipe-empty", Warning,
    "the recipe of the ammo type {type} has no ingredient", ["type"]
);
code!(
    /// The weapon names another ammo set than its custom one.
    SET_OVERRIDDEN, "ce.ammo-set-overridden", Warning,
    "the weapon asks for the ammo set {typed}, but its custom ammunition {custom} is used", ["typed", "custom"]
);
code!(
    /// The art of a type is reserved, not imported.
    ART_RESERVED, "ce.ammo-art-reserved", Info,
    "the art of {what} is {path}; put the image at {file}", ["what", "path", "file"]
);
code!(
    /// The ammunition files are not written because the install has no ammunition data.
    NO_DATA, "ce.ammo-no-data", Warning,
    "no ammunition data was found in the installed Combat Extended, so the classes and parents cannot be checked", []
);
code!(
    /// A generated def does not load as the type it should.
    DRY_LOAD_FAILED, "ce.ammo-dry-load-failed", Error,
    "{def} does not load as {expected}: {reason}", ["def", "expected", "reason"]
);
code!(
    /// A value was filled from a suggestion.
    DERIVED, "ce.ammo-derived", Info,
    "{what} of the ammo type {type} was {how}", ["type", "what", "how"]
);

code!(
    /// CEP050: an ammunition def whose parent is not known.
    CEP050, "ce.cep050-ammo-parent-unresolved", Warning,
    "{def} names the parent {parent}, which is not defined in the file and is not the parent of any installed ammunition def", ["def", "parent"]
);
code!(
    /// CEP051: an ammo set pair names an ammo item or a projectile that is not defined.
    CEP051, "ce.cep051-ammoset-pair-unresolved", Error,
    "the ammo set {def} pairs {ammo} with {projectile}, and {missing} is not defined", ["def", "ammo", "projectile", "missing"]
);
code!(
    /// CEP052: an ammo item whose cook off projectile is not defined.
    CEP052, "ce.cep052-cook-off-unresolved", Warning,
    "{def} cooks off {projectile}, which is not defined", ["def", "projectile"]
);
code!(
    /// CEP053: a recipe that makes a def that is not defined or has no ingredient.
    CEP053, "ce.cep053-recipe-broken", Error,
    "the recipe {def} {problem}", ["def", "problem"]
);
code!(
    /// CEP054: a definition of ammunition without the Combat Extended class it needs.
    CEP054, "ce.cep054-ammo-class-missing", Error,
    "{def} is {what} but has no class attribute; the game reads it as a plain thing def", ["def", "what"]
);
code!(
    /// CEP055: a projectile without Combat Extended projectile properties.
    CEP055, "ce.cep055-projectile-props-class", Error,
    "the projectile {def} has no properties with the Combat Extended class", ["def"]
);
code!(
    /// CEP056: a projectile that needs damage numbers and has none.
    CEP056, "ce.cep056-projectile-numbers-missing", Warning,
    "the projectile {def} has no {what}", ["def", "what"]
);
code!(
    /// CEP057: a def of the file with the same name as another.
    CEP057, "ce.cep057-duplicate-def-in-file", Error,
    "the def name {def} is used more than once in the file", ["def"]
);

/// Every code of this module, in declaration order.
pub const REGISTRY: &[CodeInfo] = &[
    NAME_MISSING,
    NAME_INVALID,
    CALIBER_MISSING,
    NO_TYPES,
    CLASS_MISSING,
    CLASS_UNKNOWN,
    KEY_DUPLICATE,
    FIELD_REQUIRED,
    NUMBER_INVALID,
    IMPLAUSIBLE,
    DUPLICATE_DEF,
    REF_UNRESOLVED,
    REF_DOUBTFUL,
    TEXT_INVALID,
    DEFAULT_UNKNOWN,
    NO_PARENT,
    RECIPE_EMPTY,
    SET_OVERRIDDEN,
    ART_RESERVED,
    NO_DATA,
    DRY_LOAD_FAILED,
    DERIVED,
    CEP050,
    CEP051,
    CEP052,
    CEP053,
    CEP054,
    CEP055,
    CEP056,
    CEP057,
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn codes_are_unique_well_formed_and_use_declared_arguments() {
        let mut seen = BTreeSet::new();
        for info in REGISTRY {
            assert!(seen.insert(info.code), "{}", info.code);
            assert!(info.diag_code().is_well_formed(), "{}", info.code);
            let mut rest = info.template;
            while let Some(open) = rest.find('{') {
                let after = &rest[open + 1..];
                let close = after.find('}').unwrap_or(0);
                assert!(info.args.contains(&&after[..close]), "{}", info.code);
                rest = &after[close..];
            }
        }
    }

    #[test]
    fn rule_ids_follow_the_current_last_rule() {
        let ids: Vec<String> = REGISTRY
            .iter()
            .filter_map(|c| crate::ce::lint::rule_id(c.code))
            .collect();
        assert_eq!(ids.first().map(String::as_str), Some("CEP050"));
        assert_eq!(ids.last().map(String::as_str), Some("CEP057"));
    }
}
