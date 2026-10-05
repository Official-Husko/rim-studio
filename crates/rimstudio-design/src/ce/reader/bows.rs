//! Bows and crossbows in the user's Combat Extended: how a weapon is recognised as one.
//!
//! Combat Extended converts a bow in its own style (an arrow or bolt ammo set, empty fire modes, a spawn
//! count instead of a magazine, a bow tag). A weapon counts as a bow when its shape says so, never by its
//! def name: a weapon tag or weapon class that names a bow, or a shooting verb whose projectile is an arrow
//! or a bolt. The same test serves three callers: the reader marks converted bows ([`CeGun::bow`]), the
//! convert scan lists vanilla bows as convertible, and the generator picks the bow conversion for a
//! designed weapon.

use rimstudio_core::tree::Node;

use super::CeModel;
use super::conversions::CeGun;
use crate::reader::access::{child_text, list_items, list_texts};

/// True when a projectile def name reads as an arrow or a bolt of a bow or crossbow. `Bolt` counts as a
/// whole word (`Bolt_Steel`, `NerveSpikerBolt`), so `Bullet_BoltAction` does not.
#[must_use]
pub fn is_bow_projectile(name: &str) -> bool {
    let lower = name.to_lowercase();
    if lower.contains("arrow") || lower.contains("quarrel") {
        return true;
    }
    // Words are split at non alphanumeric characters and at a lower to upper case change.
    let mut word = String::new();
    let mut previous_lower = false;
    let mut words: Vec<String> = Vec::new();
    for c in name.chars() {
        if !c.is_ascii_alphanumeric() {
            words.push(std::mem::take(&mut word));
            previous_lower = false;
            continue;
        }
        if c.is_ascii_uppercase() && previous_lower {
            words.push(std::mem::take(&mut word));
        }
        previous_lower = c.is_ascii_lowercase();
        word.push(c.to_ascii_lowercase());
    }
    words.push(word);
    words.iter().any(|w| w == "bolt") && !words.iter().any(|w| w == "action")
}

/// True when a tag or weapon class names a bow (`Bow`, `CE_Bow`, `RS_CrossbowHeavy`).
#[must_use]
pub fn names_a_bow(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("bow") && !lower.contains("elbow") && !lower.contains("rainbow")
}

/// True when the tags, the weapon classes or the default projectile of a shooting verb say bow.
#[must_use]
pub fn is_bow_shape(tags: &[String], classes: &[String], projectile: Option<&str>) -> bool {
    tags.iter().chain(classes.iter()).any(|t| names_a_bow(t))
        || projectile.is_some_and(is_bow_projectile)
}

/// The default projectile of the first verb of a def that names one.
#[must_use]
pub fn shooting_projectile(node: &Node) -> Option<String> {
    list_items(node, "verbs")
        .into_iter()
        .find_map(|v| child_text(v, "defaultProjectile"))
}

/// True when a def node has a bow shape. A def without a shooting verb is never a bow.
#[must_use]
pub fn is_bow_node(node: &Node) -> bool {
    let Some(projectile) = shooting_projectile(node) else {
        return false;
    };
    is_bow_shape(
        &list_texts(node, "weaponTags"),
        &list_texts(node, "weaponClasses"),
        Some(&projectile),
    )
}

impl CeModel {
    /// The converted bows that count as examples: bow shaped guns without an exclusion reason.
    pub fn bows(&self) -> impl Iterator<Item = &CeGun> {
        self.guns.iter().filter(|g| g.bow && g.excluded.is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("Arrow_Stone", true)]
    #[case("Projectile_StreamlinedArrow_Steel", true)]
    #[case("Bolt_Steel", true)]
    #[case("NerveSpikerBolt", true)]
    #[case("RS_Quarrel", true)]
    #[case("Bullet_BoltAction", false)]
    #[case("Bullet_Rifle", false)]
    #[case("Proj_Boltzmann", false)]
    fn projectile_names(#[case] name: &str, #[case] want: bool) {
        assert_eq!(is_bow_projectile(name), want, "{name}");
    }

    #[rstest]
    #[case("CE_Bow", true)]
    #[case("RS_CrossbowHeavy", true)]
    #[case("RS_Elbow", false)]
    #[case("Gun", false)]
    fn tag_names(#[case] name: &str, #[case] want: bool) {
        assert_eq!(names_a_bow(name), want, "{name}");
    }

    #[test]
    fn a_def_without_a_shooting_verb_is_not_a_bow() {
        let mut node = Node::new("ThingDef");
        let mut tags = Node::new("weaponTags");
        tags.push_child(Node::with_text("li", "RS_Bow"));
        node.push_child(tags);
        assert!(!is_bow_node(&node));
    }
}
