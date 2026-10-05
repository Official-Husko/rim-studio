//! Deterministic fictional reference pools for tests and demos.
//!
//! The generated items are made up: names start with `RS_`, and the numbers come from a small latent
//! model (a hidden strength drives some stats, class constants drive others) with log normal noise. They
//! resemble no game data. The generator exists so that the calibration code can be tested for its
//! qualitative behaviour (it beats the class median, its bands cover, it is monotone) on pools whose
//! structure is known.

use super::numeric::SplitMix64;
use super::{ItemKind, ReferenceItem};

/// Roles of the fictional pool.
pub const ROLES: [&str; 3] = ["RS_role_a", "RS_role_b", "RS_role_c"];
/// Number of tiers of the fictional pool.
pub const TIERS: u8 = 4;

/// Parameters of [`synthetic_items`].
#[derive(Debug, Clone, PartialEq)]
pub struct SyntheticSpec {
    /// Number of items.
    pub n: usize,
    /// Seed of the generator.
    pub seed: u64,
    /// Standard deviation of the log noise on every stat (0 gives a noise free pool).
    pub noise: f64,
    /// Standard deviation of the log strength scatter around its role and tier centre.
    pub strength_scatter: f64,
    /// Fill the explicit tier and role fields (otherwise only labels and stats are given and the pool
    /// derives the axes).
    pub explicit_axes: bool,
    /// Kind of the generated items.
    pub kind: ItemKind,
}

impl Default for SyntheticSpec {
    fn default() -> Self {
        Self {
            n: 40,
            seed: 1,
            noise: 0.08,
            strength_scatter: 0.3,
            explicit_axes: true,
            kind: ItemKind::Ranged,
        }
    }
}

/// Generates the fictional items.
///
/// Stats: `mass` (follows strength, elasticity 0.6), `damage` (follows strength, elasticity 0.8), `range`
/// and `warmup` (class constants per role), `burst` (an integer set by the group). The group is `RS_auto`
/// or `RS_semi`. Items are returned in generation order; build a pool with [`super::Pool::build`].
#[must_use]
pub fn synthetic_items(spec: &SyntheticSpec) -> Vec<ReferenceItem> {
    let mut rng = SplitMix64::new(spec.seed);
    let mut out = Vec::with_capacity(spec.n);
    for i in 0..spec.n {
        let role_idx = i % ROLES.len();
        let tier = ((i / ROLES.len()) % usize::from(TIERS)) as u8;
        let role = ROLES.get(role_idx).copied().unwrap_or("RS_role_a");
        let ln_p = 0.4 * role_idx as f64
            + 0.5 * f64::from(tier)
            + spec.strength_scatter * rng.next_normal();
        let auto = rng.next_f64() < 0.4;
        let mut noise = || spec.noise * rng.next_normal();
        let mass = (1.0 + 0.6 * ln_p + 0.1 * role_idx as f64 + noise()).exp();
        let damage = (2.0 + 0.8 * ln_p + noise()).exp();
        let range = (3.0 + 0.3 * role_idx as f64 + noise()).exp();
        let warmup = (0.6 + 0.2 * role_idx as f64) * noise().exp();
        let burst = if auto { 4.0 } else { 1.0 };
        let mut item = ReferenceItem::new(format!("RS_Item_{i:03}"), spec.kind);
        item.labels = vec![
            format!("RS_Item_{i:03}"),
            role.to_string(),
            format!("RS_tier_tag_{tier}"),
        ];
        if spec.explicit_axes {
            item.tier = Some(tier);
            item.role = Some(role.to_string());
        }
        item.group = Some(if auto { "RS_auto" } else { "RS_semi" }.to_string());
        item.strength = Some(ln_p.exp());
        item.stats.insert("mass".into(), mass);
        item.stats.insert("damage".into(), damage);
        item.stats.insert("range".into(), range);
        item.stats.insert("warmup".into(), warmup);
        item.stats.insert("burst".into(), burst);
        out.push(item);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generation_is_deterministic_and_seed_sensitive() {
        let a = synthetic_items(&SyntheticSpec::default());
        let b = synthetic_items(&SyntheticSpec::default());
        let c = synthetic_items(&SyntheticSpec {
            seed: 2,
            ..SyntheticSpec::default()
        });
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn every_item_is_fictional_and_finite() {
        for item in synthetic_items(&SyntheticSpec {
            n: 50,
            ..SyntheticSpec::default()
        }) {
            assert!(item.id.starts_with("RS_"));
            assert!(item.stats.values().all(|v| v.is_finite() && *v > 0.0));
            assert!(item.strength.is_some_and(|s| s > 0.0));
        }
    }
}
