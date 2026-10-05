//! The Combat Extended side of the archetypes: the calibres of the user's ammo sets.
//!
//! Combat Extended is optional (D-085). In vanilla mode nothing here is used. In Combat Extended mode the
//! calibre of a weapon is one of the ammo sets of the user's install: its damage against the median set of
//! its family becomes the calibre factor of the vanilla damage, and the set and its first projectile are
//! proposed for the optional block. The numbers of the block itself come from the existing predictors.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ce::ammo::{CatalogQuery, catalog};
use crate::ce::reader::CeModel;
use crate::classes::numeric::median;

/// How hard an ammo set hits compared with the median set of its family.
const MIN_RATIO: f64 = 0.4;
const MAX_RATIO: f64 = 3.0;

/// One ammo set offered as a calibre.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CeCalibre {
    /// The ammo set def name.
    pub set: String,
    /// The label of the set.
    pub label: String,
    /// The caliber text of the set.
    pub caliber: String,
    /// The family of the caliber (pistol, rifle, shotgun, ...), when the categories say.
    pub family: Option<String>,
    /// The projectile of the first ammo type.
    pub projectile: String,
    /// Damage per shot of the first ammo type (pellets included).
    pub damage: f64,
    /// Sharp penetration of the first ammo type.
    pub ap_sharp: Option<f64>,
    /// The damage against the median set of the same family (1 is typical).
    pub ratio: f64,
    /// How many converted weapons use the set.
    pub weapon_count: usize,
}

/// The Combat Extended data the archetype solver reads.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CeInfo {
    /// The ammo sets as calibres, by label.
    pub calibres: Vec<CeCalibre>,
    /// The AI class tags of the install.
    pub ai_class_tags: Vec<String>,
}

impl CeInfo {
    /// A set by def name.
    #[must_use]
    pub fn calibre(&self, set: &str) -> Option<&CeCalibre> {
        self.calibres.iter().find(|c| c.set == set)
    }
}

/// The multipliers a real calibre gives to the vanilla shape, from the ratio of its damage to the median
/// set of its family. The exponents are generic tuning constants: damage follows the ratio closely, the
/// rest of the weapon only a little.
#[must_use]
pub fn factors_of_ratio(ratio: f64) -> (f64, f64, f64, f64, f64) {
    let r = if ratio.is_finite() && ratio > 0.0 {
        ratio.clamp(MIN_RATIO, MAX_RATIO)
    } else {
        1.0
    };
    (
        r.powf(0.7),
        r.powf(0.1),
        r.powf(0.25),
        r.powf(0.15),
        r.powf(0.12),
    )
}

/// Reads the ammo sets of a Combat Extended model as calibres. Empty when Combat Extended is absent.
#[must_use]
pub fn ce_info(model: &CeModel) -> CeInfo {
    if !model.is_present() {
        return CeInfo::default();
    }
    let mut entries = Vec::new();
    let mut page = 0usize;
    loop {
        let q = CatalogQuery {
            page,
            page_size: Some(200),
            ..CatalogQuery::default()
        };
        let got = catalog(model, &q, &BTreeMap::new());
        if got.entries.is_empty() {
            break;
        }
        entries.extend(got.entries);
        page += 1;
        if page > 50 {
            break;
        }
    }
    let mut raw: Vec<CeCalibre> = entries
        .into_iter()
        .filter_map(|e| {
            let first = e.types.first()?;
            let damage = first.damage? * first.pellets.unwrap_or(1.0).max(1.0);
            (damage.is_finite() && damage > 0.0).then(|| CeCalibre {
                set: e.def_name,
                label: e.label,
                caliber: e.caliber,
                family: e.family,
                projectile: first.projectile_def.clone(),
                damage,
                ap_sharp: first.armor_penetration_sharp,
                ratio: 1.0,
                weapon_count: e.weapon_count,
            })
        })
        .collect();
    let all: Vec<f64> = raw.iter().map(|c| c.damage).collect();
    let overall = median(&all).unwrap_or(1.0);
    let mut by_family: BTreeMap<String, Vec<f64>> = BTreeMap::new();
    for c in &raw {
        by_family
            .entry(c.family.clone().unwrap_or_default().to_lowercase())
            .or_default()
            .push(c.damage);
    }
    for c in &mut raw {
        let key = c.family.clone().unwrap_or_default().to_lowercase();
        let base = by_family
            .get(&key)
            .filter(|v| v.len() >= crate::classes::MIN_POOL)
            .and_then(|v| median(v))
            .unwrap_or(overall);
        c.ratio = if base > 0.0 { c.damage / base } else { 1.0 };
    }
    raw.sort_by(|a, b| {
        a.label
            .to_lowercase()
            .cmp(&b.label.to_lowercase())
            .then_with(|| a.set.cmp(&b.set))
    });
    CeInfo {
        calibres: raw,
        ai_class_tags: model.ai_class_tags.clone(),
    }
}

/// True when the family of a calibre fits one of the hints of an archetype (compared ignoring case).
#[must_use]
pub fn family_fits(calibre: &CeCalibre, hints: &[String]) -> bool {
    let family = calibre.family.as_deref().unwrap_or_default().to_lowercase();
    let caliber = calibre.caliber.to_lowercase();
    hints.iter().any(|h| {
        let h = h.to_lowercase();
        (!family.is_empty() && family.contains(&h)) || caliber.contains(&h)
    })
}

/// The AI class tag of the install that fits an archetype: the first hint that names a tag. A hint names a
/// tag when it equals the part of the tag after its last underscore (`AR` for `CE_AI_AR`), or, for a hint of
/// four letters or more, when it is part of the tag name; both ignore case.
#[must_use]
pub fn ai_class_for(info: &CeInfo, hints: &[String]) -> Option<String> {
    hints.iter().find_map(|h| {
        let h = h.to_lowercase();
        let exact = info.ai_class_tags.iter().find(|t| {
            t.rsplit('_')
                .next()
                .is_some_and(|tail| tail.eq_ignore_ascii_case(&h))
        });
        let partial = || {
            info.ai_class_tags
                .iter()
                .find(|t| h.len() >= 4 && t.to_lowercase().contains(&h))
        };
        exact.or_else(partial).cloned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_factors_grow_with_the_ratio_and_stay_bounded() {
        let (d1, ..) = factors_of_ratio(1.0);
        let (d2, ..) = factors_of_ratio(1.5);
        let (d3, ..) = factors_of_ratio(100.0);
        assert!((d1 - 1.0).abs() < 1e-12);
        assert!(d2 > d1 && d3 > d2);
        assert!(d3 < 1.0 + MAX_RATIO);
        let (low, ..) = factors_of_ratio(0.0);
        assert_eq!(low, 1.0);
    }

    #[test]
    fn families_and_class_tags_match_by_hint() {
        let c = CeCalibre {
            set: "RS_Set".into(),
            label: "rifle round".into(),
            caliber: "5.56x45mm".into(),
            family: Some("Rifle".into()),
            projectile: "RS_Bullet".into(),
            damage: 10.0,
            ap_sharp: None,
            ratio: 1.0,
            weapon_count: 0,
        };
        assert!(family_fits(&c, &["rifle".to_owned()]));
        assert!(!family_fits(&c, &["shotgun".to_owned()]));
        let info = CeInfo {
            calibres: vec![c],
            ai_class_tags: vec!["CE_AI_AssaultRifle".into(), "CE_AI_SMG".into()],
        };
        assert_eq!(
            ai_class_for(&info, &["SMG".to_owned()]).as_deref(),
            Some("CE_AI_SMG")
        );
        assert_eq!(ai_class_for(&info, &["Sniper".to_owned()]), None);
    }
}
