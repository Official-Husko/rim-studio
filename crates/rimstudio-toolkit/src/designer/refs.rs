//! The reference list: the weapons the designer compares against, with tier, role, strength and stats.
//!
//! [`reference_list`] reads the pools of the current snapshot (vanilla pools: weapons of Combat Extended
//! never enter them) and pages them in strength order. Every number comes from the user's install. The
//! Combat Extended pool is a separate call, [`reference_list_ce`], that exists only when Combat Extended
//! data is part of the reference set; nothing in the vanilla list ever shows a CE number.

use rimstudio_design::ce::classes::{CeClassOptions, build_pool};
use rimstudio_design::classes::numeric::{finite_sorted, median, quantile_sorted};
use rimstudio_design::classes::{ItemKind as PoolKind, Pool, PoolItem};
use rimstudio_design::model::TechLevel;
use rimstudio_ipc_types::designer::{
    DesignerReferenceListRequest, ReferenceItemDto, ReferenceListDto, StatPoolDto,
};

use super::ctx::{Ctx, Engine};
use super::dto::{count, kind_from_dto, pool_kind, pool_kind_to_dto, tier_from_index, tier_to_dto};
use crate::error::{ToolkitError, ToolkitResult};

/// The most rows one call returns.
pub const MAX_LIMIT: u32 = 500;

/// The rows of one call when the request asks for zero.
pub const DEFAULT_LIMIT: u32 = 100;

/// The role the pool gives to a weapon no rule and no shared tag placed.
const NO_ROLE: &str = "any";

fn matches_filter(item: &PoolItem, req: &DesignerReferenceListRequest) -> bool {
    let role_ok = req
        .role
        .as_deref()
        .is_none_or(|r| item.role.eq_ignore_ascii_case(r));
    let tier_ok = req.tier.is_none_or(|t| item.tier == tier_from_dto_index(t));
    role_ok && tier_ok
}

fn tier_from_dto_index(t: rimstudio_ipc_types::designer::TechLevelDto) -> u8 {
    super::dto::tier_from_dto(t).index()
}

fn class_label(kind: PoolKind, req: &DesignerReferenceListRequest) -> String {
    let what = match kind {
        PoolKind::Melee => "melee weapons",
        _ => "ranged weapons",
    };
    let mut parts: Vec<String> = Vec::new();
    if let Some(t) = req.tier {
        parts.push(super::dto::tier_from_dto(t).xml_name().to_owned());
    }
    if let Some(r) = &req.role {
        parts.push(r.clone());
    }
    if parts.is_empty() {
        format!("all {what}")
    } else {
        format!("{} {what}", parts.join(" "))
    }
}

fn pools_of(items: &[&PoolItem]) -> Vec<StatPoolDto> {
    let mut names: Vec<&str> = items
        .iter()
        .flat_map(|i| i.stats.keys().map(String::as_str))
        .collect();
    names.sort_unstable();
    names.dedup();
    let mut out = Vec::new();
    for name in names {
        let values: Vec<f64> = items.iter().filter_map(|i| i.stat(name)).collect();
        let sorted = finite_sorted(&values);
        let (Some(min), Some(max), Some(p10), Some(p90), Some(med)) = (
            sorted.first().copied(),
            sorted.last().copied(),
            quantile_sorted(&sorted, 0.1),
            quantile_sorted(&sorted, 0.9),
            median(&sorted),
        ) else {
            continue;
        };
        out.push(StatPoolDto {
            stat: name.to_owned(),
            n: count(sorted.len()),
            min,
            p10,
            median: med,
            p90,
            max,
        });
    }
    out
}

fn list_pool(
    pool: &Pool,
    req: &DesignerReferenceListRequest,
    detail: &dyn Fn(&PoolItem) -> ReferenceExtras,
) -> ToolkitResult<ReferenceListDto> {
    let kind = pool.kind;
    let filtered: Vec<&PoolItem> = pool
        .items
        .iter()
        .filter(|i| matches_filter(i, req))
        .collect();
    let limit = match req.limit {
        0 => DEFAULT_LIMIT,
        n => n.min(MAX_LIMIT),
    } as usize;
    let offset = usize::try_from(req.offset).unwrap_or(usize::MAX);
    let items = filtered
        .iter()
        .enumerate()
        .skip(offset)
        .take(limit)
        .map(|(index, item)| {
            let extras = detail(item);
            ReferenceItemDto {
                index: count(index),
                def_name: item.id.clone(),
                label: item.label.clone(),
                tier: extras.tier.map(tier_to_dto),
                role: (!(item.role_derived && item.role == NO_ROLE)).then(|| item.role.clone()),
                mod_id: extras.mod_id,
                strength: Some(item.strength).filter(|s| s.is_finite()),
                stats: item.stats.clone(),
            }
        })
        .collect();
    Ok(ReferenceListDto {
        kind: pool_kind_to_dto(kind)?,
        class_label: class_label(kind, req),
        total: count(filtered.len()),
        offset: req.offset,
        items,
        pools: pools_of(&filtered),
    })
}

struct ReferenceExtras {
    tier: Option<TechLevel>,
    mod_id: Option<String>,
}

fn vanilla_extras(engine: &Engine, item: &PoolItem) -> ReferenceExtras {
    let weapon = engine
        .pools()
        .ok()
        .and_then(|p| p.set.weapon(&item.id).cloned());
    ReferenceExtras {
        tier: weapon.as_ref().and_then(|w| w.tech_level),
        mod_id: weapon
            .as_ref()
            .and_then(|w| w.mod_idx)
            .and_then(|m| engine.mod_id_of(m))
            .map(str::to_owned),
    }
}

/// The reference weapons of a kind, optionally of one role and tier, in strength order with the pool
/// distribution of every stat (`designer_reference_list`).
///
/// # Errors
///
/// [`ToolkitError::ReferenceUnavailable`] without a game install or without a weapon database.
pub fn reference_list(
    ctx: &Ctx,
    req: DesignerReferenceListRequest,
) -> ToolkitResult<ReferenceListDto> {
    let engine = ctx.require_engine()?;
    let kind = pool_kind(kind_from_dto(req.kind))?;
    let pool = engine.pool(kind)?;
    list_pool(pool, &req, &|item| vanilla_extras(&engine, item))
}

/// The converted weapons of Combat Extended as a reference list, for the optional patch.
///
/// The items carry the Combat Extended numbers and, where a vanilla twin exists, its numbers under
/// `vanilla.<stat>` names. It is only available when Combat Extended data is part of the reference set.
///
/// # Errors
///
/// [`ToolkitError::ReferenceUnavailable`] when Combat Extended is not part of the reference set.
pub fn reference_list_ce(
    ctx: &Ctx,
    req: DesignerReferenceListRequest,
) -> ToolkitResult<ReferenceListDto> {
    let engine = ctx.require_engine()?;
    let model = engine.ce();
    if !model.is_present() {
        return Err(ToolkitError::ReferenceUnavailable {
            reason: model
                .absent
                .clone()
                .unwrap_or_else(|| "no Combat Extended found".to_owned()),
        });
    }
    let kind = pool_kind(kind_from_dto(req.kind))?;
    let pool = build_pool(model, kind, &CeClassOptions::default())?;
    list_pool(&pool, &req, &|item| ReferenceExtras {
        tier: tier_from_index(item.tier),
        mod_id: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_design::classes::PoolOptions;
    use rimstudio_design::classes::synthetic::{SyntheticSpec, synthetic_items};
    use rimstudio_ipc_types::designer::{ItemKindDto, TechLevelDto};

    fn pool() -> Pool {
        let items = synthetic_items(&SyntheticSpec {
            n: 30,
            ..SyntheticSpec::default()
        });
        Pool::build(PoolKind::Ranged, &items, &PoolOptions::default())
    }

    fn no_extras(_: &PoolItem) -> ReferenceExtras {
        ReferenceExtras {
            tier: None,
            mod_id: None,
        }
    }

    #[test]
    fn the_class_label_names_the_filters() {
        let mut req = DesignerReferenceListRequest::default();
        assert_eq!(class_label(PoolKind::Ranged, &req), "all ranged weapons");
        req.tier = Some(TechLevelDto::Spacer);
        req.role = Some("rifle".into());
        req.kind = ItemKindDto::Melee;
        assert_eq!(
            class_label(PoolKind::Melee, &req),
            "Spacer rifle melee weapons"
        );
    }

    #[test]
    fn listing_a_pool_pages_and_caps_the_limit() {
        let pool = pool();
        let mut req = DesignerReferenceListRequest {
            limit: 0,
            ..DesignerReferenceListRequest::default()
        };
        let list = list_pool(&pool, &req, &no_extras).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(list.total, 30);
        assert_eq!(list.items.len(), 30, "zero means the default of 100");
        req.limit = 1_000_000;
        req.offset = 28;
        let list = list_pool(&pool, &req, &no_extras).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].index, 28);
        req.offset = u32::MAX;
        let list = list_pool(&pool, &req, &no_extras).unwrap_or_else(|e| panic!("{e}"));
        assert!(list.items.is_empty());
        assert_eq!(list.total, 30);
    }

    #[test]
    fn pool_distributions_are_ordered_and_ignore_missing_values() {
        let pool = pool();
        let items: Vec<&PoolItem> = pool.items.iter().collect();
        let pools = pools_of(&items);
        assert!(!pools.is_empty());
        for p in &pools {
            assert!(p.min <= p.p10 && p.p10 <= p.median && p.median <= p.p90 && p.p90 <= p.max);
        }
        assert!(pools_of(&[]).is_empty());
    }
}
