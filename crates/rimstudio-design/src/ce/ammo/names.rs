//! The class names of the ammunition defs of Combat Extended, and the def names RimStudio derives for a
//! custom caliber.
//!
//! The class strings live in this module only (`xtask check-source`, rule `source.ce-class`). The derived
//! names follow the spelling of the user's own ammunition (`Ammo_<caliber>_<class>`, `Bullet_...`,
//! `AmmoSet_...`, `MakeAmmo_...`) with the project prefix in front, so the project prefix rule of the
//! designer holds for every def written here.

use crate::model::{CustomAmmoSpec, CustomAmmoType};

/// The full type name of an ammo item def (a thing def subclass).
pub const AMMO_DEF: &str = "CombatExtended.AmmoDef";
/// The full type name of an ammo category def (the ammo class: full metal jacket, armor piercing, ...).
pub const AMMO_CATEGORY_DEF: &str = "CombatExtended.AmmoCategoryDef";
/// The class attribute of the projectile properties.
pub const PROJECTILE_PROPS: &str = "CombatExtended.ProjectilePropertiesCE";
/// The class attribute of the fragments component of a projectile.
pub const FRAGMENTS_COMP: &str = "CombatExtended.CompProperties_Fragments";

/// The def types of the ammunition that a type table needs besides the ones of the ammo set def, as
/// `(full name, base)` entries.
#[must_use]
pub fn type_entries() -> Vec<(String, String)> {
    vec![
        (AMMO_DEF.to_owned(), "Verse.ThingDef".to_owned()),
        (AMMO_CATEGORY_DEF.to_owned(), "Verse.Def".to_owned()),
    ]
}

/// The element and attribute names of the ammo set pairs: `<AmmoDef>Projectile</AmmoDef>` or `li` entries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PairForm {
    /// One element per pair: the ammo def name is the element name and the projectile the text.
    #[default]
    Element,
    /// A list entry per pair with `ammo` and `projectile` children.
    Li,
}

/// Keeps the characters of an XML name and drops the rest; a name that would start with a digit gets an
/// underscore in front.
#[must_use]
pub fn identifier(text: &str) -> String {
    let mut out: String = text
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// The def names derived for a custom caliber.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedNames {
    /// `<prefix>_` or the empty string.
    prefix: String,
    /// The identifier made from the name of the spec.
    name: String,
}

impl DerivedNames {
    /// The names of a spec under a project prefix (without the trailing underscore).
    #[must_use]
    pub fn new(spec: &CustomAmmoSpec, prefix: &str) -> Self {
        let prefix = if prefix.is_empty() {
            String::new()
        } else {
            format!("{prefix}_")
        };
        Self {
            prefix,
            name: identifier(&spec.name),
        }
    }

    /// The ammo set def name.
    #[must_use]
    pub fn set(&self) -> String {
        format!("{}AmmoSet_{}", self.prefix, self.name)
    }

    /// The thing category def name of the caliber.
    #[must_use]
    pub fn category(&self) -> String {
        format!("{}Ammo{}", self.prefix, self.name)
    }

    /// The ammo item def name of a type.
    #[must_use]
    pub fn ammo(&self, key: &str) -> String {
        format!("{}Ammo_{}_{}", self.prefix, self.name, identifier(key))
    }

    /// The projectile def name of a type.
    #[must_use]
    pub fn projectile(&self, key: &str) -> String {
        format!("{}Bullet_{}_{}", self.prefix, self.name, identifier(key))
    }

    /// The recipe def name of a type.
    #[must_use]
    pub fn recipe(&self, key: &str) -> String {
        format!("{}MakeAmmo_{}_{}", self.prefix, self.name, identifier(key))
    }

    /// The file stem of the definition file of the caliber.
    #[must_use]
    pub fn file_stem(&self) -> String {
        format!("{}{}", self.prefix, self.name)
    }
}

/// The key of a type: the key as typed, else the identifier of its ammo class.
#[must_use]
pub fn type_key(t: &CustomAmmoType) -> String {
    if t.key.trim().is_empty() {
        identifier(&t.ammo_class)
    } else {
        identifier(&t.key)
    }
}
