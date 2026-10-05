//! Custom Combat Extended ammunition: the catalogue of the ammo sets of the user's install
//! (`designer_ce_ammo_catalog`), suggestions for a new ammo type (`designer_ce_ammo_suggest`) and the custom
//! ammo spec that the Combat Extended block of a draft can carry (`ce.customAmmo`).
//!
//! A custom caliber is an ammo set with ammo types. Each type has the data of a projectile, of the ammo
//! item that carries it and of the recipe that makes it. Every number is a sourced value (typed, suggested,
//! answered), so a value the user typed is never replaced by a suggestion. The JSON shapes are identical to
//! the design engine's own types; the toolkit converts by serialising one and deserialising the other.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::carried::RawNodeDto;
use super::ce_suggest::CeRatingDto;
use super::draft::DraftDto;
use super::spec::SourcedDto;

/// What a burning ammo item does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum CookOffKindDto {
    /// The item fires its projectile in a random direction.
    Projectile,
    /// The item detonates its projectile where it lies.
    Detonate,
    /// Nothing special happens.
    None,
}

/// A secondary damage entry of a projectile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CustomSecondaryDamageDto {
    /// The damage def name.
    pub def: String,
    /// The damage amount.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub amount: Option<SourcedDto<f64>>,
}

/// A fragment a projectile scatters when it bursts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CustomFragmentDto {
    /// The fragment projectile def name.
    pub def: String,
    /// How many of them.
    pub count: u32,
}

/// The data of one projectile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CustomProjectileDto {
    /// The label of the projectile def. Derived from the ammo label when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub label: Option<String>,
    /// The abstract def the projectile inherits from.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub parent: Option<String>,
    /// The `thingClass` of the projectile.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub thing_class: Option<String>,
    /// The damage def.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub damage_def: Option<String>,
    /// Damage per projectile.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub damage: Option<SourcedDto<f64>>,
    /// Sharp armor penetration.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub armor_penetration_sharp: Option<SourcedDto<f64>>,
    /// Blunt armor penetration.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub armor_penetration_blunt: Option<SourcedDto<f64>>,
    /// Muzzle speed in cells per second.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub speed: Option<SourcedDto<f64>>,
    /// Pellets per shot.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub pellet_count: Option<SourcedDto<f64>>,
    /// The spread multiplier of the pellets.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub spread_mult: Option<SourcedDto<f64>>,
    /// Secondary damage entries.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(
        feature = "ts",
        ts(as = "Option<Vec<CustomSecondaryDamageDto>>", optional)
    )]
    pub secondary_damage: Vec<CustomSecondaryDamageDto>,
    /// Explosion radius in cells.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub explosion_radius: Option<SourcedDto<f64>>,
    /// Whether the explosion also damages the cells next to its radius.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub explosion_neighbors: Option<bool>,
    /// Suppression factor.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub suppression_factor: Option<SourcedDto<f64>>,
    /// Danger factor.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub danger_factor: Option<SourcedDto<f64>>,
    /// Whether firing it drops a casing.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub drops_casings: Option<bool>,
    /// The casing fleck def.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub casing_mote: Option<String>,
    /// The casing filth def.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub casing_filth: Option<String>,
    /// The projectile starts fires.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub incendiary: Option<bool>,
    /// The projectile flies over its target before it bursts.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub fly_overhead: Option<bool>,
    /// The texture path.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tex_path: Option<String>,
    /// The graphic class.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub graphic_class: Option<String>,
    /// The draw size as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub draw_size: Option<String>,
    /// Other children of the graphic data, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub graphic_extra: Vec<RawNodeDto>,
    /// The sound of the explosion.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sound_explode: Option<String>,
    /// The sound in flight.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sound_ambient: Option<String>,
    /// The sound of an impact on a thick roof.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sound_hit_thick_roof: Option<String>,
    /// The sound played before the impact.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub sound_impact_anticipate: Option<String>,
    /// Fragments scattered on a burst.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CustomFragmentDto>>", optional))]
    pub fragments: Vec<CustomFragmentDto>,
    /// Other children of the projectile properties, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub extra: Vec<RawNodeDto>,
    /// Other components and children of the projectile def, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub thing_extra: Vec<RawNodeDto>,
}

/// The ammo item that carries a projectile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CustomAmmoItemDto {
    /// The abstract def the item inherits from.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub parent: Option<String>,
    /// Mass of one item.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub mass: Option<SourcedDto<f64>>,
    /// Bulk of one item.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub bulk: Option<SourcedDto<f64>>,
    /// Market value. Absent: the game computes it from the recipe.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub market_value: Option<SourcedDto<f64>>,
    /// The stack limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub stack_limit: Option<SourcedDto<u32>>,
    /// Thing categories besides the category of the set.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub thing_categories: Vec<String>,
    /// Trade tags.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub trade_tags: Vec<String>,
    /// The texture path.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tex_path: Option<String>,
    /// The graphic class.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub graphic_class: Option<String>,
    /// The draw size as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub draw_size: Option<String>,
    /// Other children of the graphic data, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub graphic_extra: Vec<RawNodeDto>,
    /// Stats besides mass, bulk and market value, by stat def name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[cfg_attr(
        feature = "ts",
        ts(as = "Option<BTreeMap<String, SourcedDto<f64>>>", optional)
    )]
    pub stat_bases: BTreeMap<String, SourcedDto<f64>>,
    /// The tech level name.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub tech_level: Option<String>,
    /// What the item does when it burns. Absent is derived from the projectile.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub cook_off: Option<CookOffKindDto>,
    /// Other children of the item def, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub extra: Vec<RawNodeDto>,
}

/// One ingredient of an ammo recipe.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CustomIngredientDto {
    /// The thing def name.
    pub thing: String,
    /// How many are needed for one craft.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub count: Option<SourcedDto<f64>>,
    /// Other thing defs the ingredient accepts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub alternatives: Vec<String>,
    /// Categories the ingredient accepts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub categories: Vec<String>,
}

/// The recipe that makes an ammo item.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CustomAmmoRecipeDto {
    /// The abstract recipe def the recipe inherits from.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub parent: Option<String>,
    /// The label of the recipe.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub label: Option<String>,
    /// The description of the recipe.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub description: Option<String>,
    /// The job string of the recipe.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub job_string: Option<String>,
    /// The ingredients of one craft.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<CustomIngredientDto>>", optional))]
    pub ingredients: Vec<CustomIngredientDto>,
    /// Items made per craft.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub products: Option<SourcedDto<u32>>,
    /// The work amount of one craft.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub work_amount: Option<SourcedDto<f64>>,
    /// The workbenches that offer the recipe.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub users: Vec<String>,
    /// The research project that unlocks the recipe. An empty text clears the parent's.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub research_prerequisite: Option<String>,
    /// Several research projects that must all be done.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<String>>", optional))]
    pub research_prerequisites: Vec<String>,
    /// Other children of the recipe def, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub extra: Vec<RawNodeDto>,
    /// The Crafting skill level the recipe needs.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub skill_level: Option<SourcedDto<u32>>,
}

/// One ammo type: an ammo class with its projectile, its item and its recipe.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CustomAmmoTypeDto {
    /// The short key that ends the def names of the type. Derived from the ammo class when empty.
    pub key: String,
    /// The ammo class def name.
    pub ammo_class: String,
    /// The label of the ammo item.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub label: Option<String>,
    /// The description of the ammo item.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub description: Option<String>,
    /// The ammo def this type was started from (information only).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub copied_from: Option<String>,
    /// The projectile.
    pub projectile: CustomProjectileDto,
    /// The ammo item.
    pub item: CustomAmmoItemDto,
    /// The recipe.
    pub recipe: CustomAmmoRecipeDto,
}

/// A custom caliber: the ammo set and its types. Part of the Combat Extended block (`ce.customAmmo`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CustomAmmoDto {
    /// The name the def names derive from.
    pub name: String,
    /// The caliber label.
    pub caliber: String,
    /// The label of the ammo set def.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub set_label: Option<String>,
    /// The generic ammo set this one is similar to.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub similar_to: Option<String>,
    /// The parent thing category of the caliber's own category.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub category_parent: Option<String>,
    /// The icon of the caliber's thing category.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub category_icon: Option<String>,
    /// The key of the type whose projectile is the weapon's default projectile.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub default_type: Option<String>,
    /// Other children of the ammo set def, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(feature = "ts", ts(as = "Option<Vec<RawNodeDto>>", optional))]
    pub set_extra: Vec<RawNodeDto>,
    /// The ammo types, in set order.
    pub types: Vec<CustomAmmoTypeDto>,
}

/// Request of `designer_ce_ammo_catalog`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct DesignerCeAmmoCatalogRequest {
    /// The current design. With it the sets the designer would suggest for it are marked; without it none
    /// is.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub draft: Option<DraftDto>,
    /// Words that must all appear in a set.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub query: Option<String>,
    /// A caliber or family, as the facets name them.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub caliber: Option<String>,
    /// An ammo class def name.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub class: Option<String>,
    /// The page, counted from zero.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub page: Option<u32>,
    /// Entries per page; the default is 25.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub page_size: Option<u32>,
}

/// A secondary damage entry of a catalogue type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeAmmoSecondaryDto {
    /// The damage def.
    pub def: String,
    /// The amount.
    pub amount: f64,
}

/// One ammo type of a catalogue entry.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CeAmmoTypeDto {
    /// The ammo item def name.
    pub ammo_def: String,
    /// The label of the ammo item.
    pub ammo_label: String,
    /// The ammo class def name.
    pub ammo_class: String,
    /// The label of the ammo class.
    pub ammo_class_label: String,
    /// The projectile def name.
    pub projectile_def: String,
    /// The damage def of the projectile.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub damage_def: Option<String>,
    /// Damage per projectile.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub damage: Option<f64>,
    /// Sharp penetration.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub armor_penetration_sharp: Option<f64>,
    /// Blunt penetration.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub armor_penetration_blunt: Option<f64>,
    /// Speed in cells per second.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub speed: Option<f64>,
    /// Pellets per shot.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub pellets: Option<f64>,
    /// Explosion radius.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub explosion_radius: Option<f64>,
    /// Secondary damage entries.
    pub secondary_damage: Vec<CeAmmoSecondaryDto>,
}

/// One ammo set of the catalogue.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CeAmmoEntryDto {
    /// The def name of the set.
    pub def_name: String,
    /// The label of the set.
    pub label: String,
    /// The caliber group.
    pub caliber: String,
    /// The family of the caliber.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub family: Option<String>,
    /// The set this one is similar to.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub similar_to: Option<String>,
    /// True when other sets are similar to this one.
    pub generic: bool,
    /// How many sets are similar to this one.
    pub similar_sets: u32,
    /// The ammo types.
    pub types: Vec<CeAmmoTypeDto>,
    /// How many converted weapons use the set.
    pub weapon_count: u32,
    /// A few labels of those weapons.
    pub example_weapons: Vec<String>,
    /// True when the relevance ranking of the current design suggests the set.
    pub suggested: bool,
    /// The relevance score, when suggested.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub score: Option<f64>,
}

/// A value of a catalogue facet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeAmmoFacetDto {
    /// The value to send back as the filter.
    pub name: String,
    /// What to show.
    pub label: String,
    /// How many sets carry it.
    pub count: u32,
}

/// Response of `designer_ce_ammo_catalog`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CeAmmoCatalogDto {
    /// False when no Combat Extended data is loaded; `reason` says why.
    pub available: bool,
    /// The plain reason when `available` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reason: Option<String>,
    /// Sets in the install.
    pub total: u32,
    /// Sets that match the query and the filters.
    pub matching: u32,
    /// The page returned.
    pub page: u32,
    /// Entries per page.
    pub page_size: u32,
    /// The entries of the page.
    pub entries: Vec<CeAmmoEntryDto>,
    /// Calibers and families of the sets that match the search words.
    pub calibers: Vec<CeAmmoFacetDto>,
    /// Ammo classes of the sets that match the search words.
    pub classes: Vec<CeAmmoFacetDto>,
}

/// What the user knows about the caliber they are making.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct CeAmmoHintsDto {
    /// A caliber name (information).
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub caliber: Option<String>,
    /// The damage wanted.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub damage: Option<f64>,
    /// The speed wanted.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub speed: Option<f64>,
    /// An existing ammo set whose ammunition is the yardstick.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub similar_set: Option<String>,
}

/// Request of `designer_ce_ammo_suggest`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase", default)]
pub struct DesignerCeAmmoSuggestRequest {
    /// The ammo class def name of the new type.
    pub class: String,
    /// What the user knows about the caliber.
    pub hints: CeAmmoHintsDto,
    /// An ammo def to start the type from: every value is copied.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub copy_from: Option<String>,
}

/// Where a suggested value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum CeAmmoSourceDto {
    /// Copied from the ammunition the user started from.
    Copied,
    /// The median of the nearest ammunition of the same class.
    Nearest,
    /// The user's hint, used as given.
    Hint,
}

/// An existing ammo type near the one asked for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeNearestAmmoDto {
    /// The ammo item def name.
    pub ammo_def: String,
    /// Its label.
    pub ammo_label: String,
    /// The ammo set it was found in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub set: Option<String>,
    /// The projectile def name.
    pub projectile_def: String,
    /// Damage.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub damage: Option<f64>,
    /// Speed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub speed: Option<f64>,
    /// The energy index (damage times speed).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub energy: Option<f64>,
    /// Distance from the target on the energy scale.
    pub distance: f64,
}

/// One suggested field of the new type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeAmmoSuggestedFieldDto {
    /// The JSON pointer of the field inside the type.
    pub field: String,
    /// What the field is called.
    pub label: String,
    /// The number, for a numeric field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub value: Option<f64>,
    /// The text, for a text field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub text: Option<String>,
    /// Where it comes from.
    pub source: CeAmmoSourceDto,
    /// How far it can be trusted.
    pub rating: CeRatingDto,
    /// How many ammunition types it rests on.
    pub n: u32,
    /// The ammo defs it rests on.
    pub from: Vec<String>,
    /// The smallest and the largest number among the neighbours.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub range: Option<(f64, f64)>,
}

/// Response of `designer_ce_ammo_suggest`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CeAmmoSuggestionDto {
    /// False when there is nothing to suggest from; `reason` says why.
    pub available: bool,
    /// The plain reason when `available` is false.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub reason: Option<String>,
    /// The ammo class asked for.
    pub class: String,
    /// The label of the class.
    pub class_label: String,
    /// The ammo def the type was copied from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "ts", ts(optional))]
    pub copied_from: Option<String>,
    /// The nearest ammunition, nearest first.
    pub nearest: Vec<CeNearestAmmoDto>,
    /// The suggested fields with their sources and ratings.
    pub fields: Vec<CeAmmoSuggestedFieldDto>,
    /// A type filled with the suggestions.
    pub ammo_type: CustomAmmoTypeDto,
    /// Remarks.
    pub notes: Vec<String>,
}
