//! Custom Combat Extended ammunition: a new ammo set with its own projectiles, ammo items and recipes.
//!
//! The optional Combat Extended block of a spec can carry a [`CustomAmmoSpec`]. It describes a caliber (an
//! ammo set) and a list of ammo types. Each type has the data of a Combat Extended projectile, of the ammo
//! item that carries it and of the recipe that makes the item. The weapon's conversion then uses the custom
//! set as its ammo set and the projectile of the default type as its default projectile.
//!
//! Everything here is plain data with `serde` support. Every number is a [`Sourced`] value, so a value the
//! user typed is never replaced by a suggestion (IT-003). Names are derived from [`CustomAmmoSpec::name`] and
//! the project prefix by [`crate::ce::ammo::names`]; this module holds no Combat Extended class name and no
//! game value. The files are produced by [`crate::ce::ammo`] and only when the Combat Extended switch is on.

use std::collections::BTreeMap;

use rimstudio_core::tree::Node;
use serde::{Deserialize, Serialize};

use super::source::Sourced;

/// What a destroyed ammo item does when it burns (the cook off behaviour of the item).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CookOffKind {
    /// The item fires its projectile in a random direction (a bullet, a shell).
    Projectile,
    /// The item detonates its projectile where it lies (a grenade, a warhead).
    Detonate,
    /// The item does nothing special.
    None,
}

/// A secondary damage entry of a projectile: a damage def and its amount, applied besides the main damage
/// (an incendiary core, an explosive charge).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomSecondaryDamage {
    /// The damage def name.
    pub def: String,
    /// The damage amount.
    pub amount: Option<Sourced<f64>>,
}

/// A fragment the projectile scatters when it bursts.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomFragment {
    /// The fragment projectile def name.
    pub def: String,
    /// How many of them.
    pub count: u32,
}

/// The data of one projectile: every field the projectiles of the user's Combat Extended carry that has a
/// typed place here; anything else goes into [`CustomProjectile::extra`] and [`CustomProjectile::thing_extra`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomProjectile {
    /// The label of the projectile def. Derived from the ammo label when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The abstract def the projectile inherits from (`ParentName`), taken from the user's own projectiles.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The `thingClass` of the projectile (a bullet, an explosive shell). Absent keeps the parent's.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thing_class: Option<String>,
    /// The damage def of the projectile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage_def: Option<String>,
    /// Damage per projectile (`damageAmountBase`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub damage: Option<Sourced<f64>>,
    /// Sharp armor penetration in millimetres of rolled steel equivalent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub armor_penetration_sharp: Option<Sourced<f64>>,
    /// Blunt armor penetration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub armor_penetration_blunt: Option<Sourced<f64>>,
    /// Muzzle speed in cells per second.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speed: Option<Sourced<f64>>,
    /// Pellets per shot (a shotgun shell fires several).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pellet_count: Option<Sourced<f64>>,
    /// The spread multiplier of the pellets.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spread_mult: Option<Sourced<f64>>,
    /// Secondary damage entries.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub secondary_damage: Vec<CustomSecondaryDamage>,
    /// Explosion radius in cells; absent for a projectile that does not explode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explosion_radius: Option<Sourced<f64>>,
    /// Whether the explosion also damages the cells next to its radius.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub explosion_neighbors: Option<bool>,
    /// How much the projectile suppresses its target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suppression_factor: Option<Sourced<f64>>,
    /// How dangerous the projectile looks to pawns that choose a position.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub danger_factor: Option<Sourced<f64>>,
    /// Whether firing it drops a casing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drops_casings: Option<bool>,
    /// The casing fleck def.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub casing_mote: Option<String>,
    /// The casing filth def.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub casing_filth: Option<String>,
    /// The projectile starts fires (a flag the AI reads).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incendiary: Option<bool>,
    /// The projectile flies over the target before it bursts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fly_overhead: Option<bool>,
    /// The texture path of the projectile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tex_path: Option<String>,
    /// The graphic class of the projectile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphic_class: Option<String>,
    /// The draw size, as written (`(0.5,0.5)`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draw_size: Option<String>,
    /// Other children of the graphic data (a colour, a shader), as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub graphic_extra: Vec<Node>,
    /// The sound of the explosion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_explode: Option<String>,
    /// The sound of the projectile in flight.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_ambient: Option<String>,
    /// The sound of an impact on a thick roof.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_hit_thick_roof: Option<String>,
    /// The sound played before the impact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sound_impact_anticipate: Option<String>,
    /// Fragments scattered on a burst.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fragments: Vec<CustomFragment>,
    /// Other children of the projectile properties, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Node>,
    /// Other components (`li` entries of `comps`) and children of the projectile def, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub thing_extra: Vec<Node>,
}

/// The ammo item that carries a projectile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomAmmoItem {
    /// The abstract def the item inherits from, taken from the user's own ammo.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The `Mass` stat of one item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mass: Option<Sourced<f64>>,
    /// The `Bulk` stat of one item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bulk: Option<Sourced<f64>>,
    /// The market value. Absent: the game computes it from the recipe, as it does for Combat Extended's own
    /// ammunition.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub market_value: Option<Sourced<f64>>,
    /// The stack limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stack_limit: Option<Sourced<u32>>,
    /// Thing categories besides the category of the set.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub thing_categories: Vec<String>,
    /// Trade tags (the ones the ammo injector reads decide whether the item is traded and crafted).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub trade_tags: Vec<String>,
    /// The texture path of the item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tex_path: Option<String>,
    /// The graphic class of the item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graphic_class: Option<String>,
    /// The draw size of the item, as written.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub draw_size: Option<String>,
    /// Other children of the graphic data (a colour, a shader), as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub graphic_extra: Vec<Node>,
    /// Stats besides mass, bulk and market value, by stat def name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub stat_bases: BTreeMap<String, Sourced<f64>>,
    /// The tech level of the item, a name of the tech level enumeration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tech_level: Option<String>,
    /// What the item does when it burns. Absent is derived from the projectile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cook_off: Option<CookOffKind>,
    /// Other children of the item def, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Node>,
}

/// One ingredient of an ammo recipe.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomIngredient {
    /// The thing def name of the ingredient.
    pub thing: String,
    /// How many are needed for one craft.
    pub count: Option<Sourced<f64>>,
    /// Other thing defs the ingredient accepts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alternatives: Vec<String>,
    /// Stuff or thing categories the ingredient accepts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<String>,
}

/// The recipe that makes an ammo item.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomAmmoRecipe {
    /// The abstract recipe def the recipe inherits from, taken from the user's own ammo recipes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<String>,
    /// The label of the recipe. Derived from the ammo label when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The description of the recipe. Derived when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The job string of the recipe. Derived when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub job_string: Option<String>,
    /// The ingredients of one craft.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub ingredients: Vec<CustomIngredient>,
    /// How many items one craft makes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub products: Option<Sourced<u32>>,
    /// The work amount of one craft.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub work_amount: Option<Sourced<f64>>,
    /// The workbenches that offer the recipe (`recipeUsers`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub users: Vec<String>,
    /// The research project that unlocks the recipe.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub research_prerequisite: Option<String>,
    /// Several research projects that must all be done (`researchPrerequisites`).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub research_prerequisites: Vec<String>,
    /// Other children of the recipe def, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extra: Vec<Node>,
    /// The Crafting skill level the recipe needs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skill_level: Option<Sourced<u32>>,
}

/// One ammo type: an ammo class with its projectile, its item and its recipe.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomAmmoType {
    /// The short key that ends the def names of the type (`FMJ`). Derived from the ammo class when empty.
    pub key: String,
    /// The ammo class: the def name of an ammo category def of the user's Combat Extended. REQ.
    pub ammo_class: String,
    /// The label of the ammo item. Derived from the caliber and the class when absent.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The description of the ammo item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The ammo def this type was started from (information; changes nothing).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copied_from: Option<String>,
    /// The projectile.
    pub projectile: CustomProjectile,
    /// The ammo item.
    pub item: CustomAmmoItem,
    /// The recipe.
    pub recipe: CustomAmmoRecipe,
}

/// A custom caliber: the ammo set and its types.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CustomAmmoSpec {
    /// The name the def names derive from (`Foo762`). With the project prefix it gives `<prefix>_AmmoSet_Foo762`.
    pub name: String,
    /// The caliber label shown in game. REQ.
    pub caliber: String,
    /// The label of the ammo set def. Defaults to the caliber.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub set_label: Option<String>,
    /// The generic ammo set this one is similar to (`similarTo`), so guns of a generic set can use it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub similar_to: Option<String>,
    /// The parent thing category of the caliber's own category (`AmmoRifles` and the like).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_parent: Option<String>,
    /// The icon of the caliber's thing category.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_icon: Option<String>,
    /// The key of the type whose projectile is the weapon's default projectile. Absent is the first type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_type: Option<String>,
    /// Other children of the ammo set def, as written.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub set_extra: Vec<Node>,
    /// The ammo types, in the order of the set.
    pub types: Vec<CustomAmmoType>,
}
