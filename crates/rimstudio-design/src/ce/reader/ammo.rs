//! Ammo sets and the projectiles they fire, read from the user's Combat Extended defs.

use std::collections::BTreeMap;

use rimstudio_core::tree::Node;
use rimstudio_defs::DefDatabases;
use serde::{Deserialize, Serialize};

use crate::reader::access::{child_number, child_text, text_of};

/// The numbers of one projectile.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectileInfo {
    /// Damage per projectile.
    pub damage: Option<f64>,
    /// Sharp penetration (mm of rolled steel equivalent).
    pub ap_sharp: Option<f64>,
    /// Blunt penetration.
    pub ap_blunt: Option<f64>,
    /// Speed in cells per second.
    pub speed: Option<f64>,
    /// Projectiles per shot (pellets).
    pub pellets: Option<f64>,
    /// Explosion radius, when the projectile explodes.
    pub explosion_radius: Option<f64>,
}

impl ProjectileInfo {
    /// Reads the `projectile` element of a projectile def.
    #[must_use]
    pub fn from_def(node: &Node) -> Self {
        let Some(p) = node.child("projectile") else {
            return Self::default();
        };
        Self {
            damage: child_number(p, "damageAmountBase"),
            ap_sharp: child_number(p, "armorPenetrationSharp"),
            ap_blunt: child_number(p, "armorPenetrationBlunt"),
            speed: child_number(p, "speed"),
            pellets: child_number(p, "pelletCount"),
            explosion_radius: child_number(p, "explosionRadius"),
        }
    }
}

/// One entry of an ammo set: an ammo item and the projectile it fires.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmmoType {
    /// Def name of the ammo item.
    pub ammo: String,
    /// Def name of the projectile.
    pub projectile: String,
    /// The projectile's numbers (empty when the projectile def is not loaded).
    pub info: ProjectileInfo,
}

/// An ammo set def with its members.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AmmoSetInfo {
    /// Def name of the set.
    pub def_name: String,
    /// The set this one is similar to, when given.
    pub similar_to: Option<String>,
    /// The members in def order.
    pub ammo_types: Vec<AmmoType>,
}

impl AmmoSetInfo {
    /// True when the projectile is a member of the set.
    #[must_use]
    pub fn has_projectile(&self, projectile: &str) -> bool {
        self.ammo_types.iter().any(|a| a.projectile == projectile)
    }

    /// The first member, which converted guns are measured by.
    #[must_use]
    pub fn first(&self) -> Option<&AmmoType> {
        self.ammo_types.first()
    }
}

fn members(node: &Node) -> Vec<(String, String)> {
    let Some(list) = node.child("ammoTypes") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in list.elements() {
        if entry.tag == "li" {
            if let (Some(ammo), Some(proj)) =
                (child_text(entry, "ammo"), child_text(entry, "projectile"))
            {
                out.push((ammo, proj));
            }
        } else if let Some(proj) = text_of(entry) {
            out.push((entry.tag.clone(), proj));
        }
    }
    out
}

/// Reads every ammo set of the database `set_type`, resolving projectiles in the database `thing_type`.
/// An unknown set type gives an empty list.
#[must_use]
pub fn read_ammo_sets(dbs: &DefDatabases, set_type: &str, thing_type: &str) -> Vec<AmmoSetInfo> {
    let Ok(view) = dbs.database(set_type) else {
        return Vec::new();
    };
    let mut cache: BTreeMap<String, ProjectileInfo> = BTreeMap::new();
    let mut out = Vec::new();
    for def in view.iter() {
        let ammo_types = members(&def.node)
            .into_iter()
            .map(|(ammo, projectile)| {
                let info = cache
                    .entry(projectile.clone())
                    .or_insert_with(|| {
                        dbs.get(thing_type, &projectile)
                            .map(|d| ProjectileInfo::from_def(&d.node))
                            .unwrap_or_default()
                    })
                    .clone();
                AmmoType {
                    ammo,
                    projectile,
                    info,
                }
            })
            .collect();
        out.push(AmmoSetInfo {
            def_name: def.def_name.clone(),
            similar_to: child_text(&def.node, "similarTo"),
            ammo_types,
        });
    }
    out
}
