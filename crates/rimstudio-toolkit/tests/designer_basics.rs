//! Smoke test of the fixture: the session loads, the pools hold the fictional weapons.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use rimstudio_design::classes::ItemKind;

#[test]
fn the_fixture_builds_pools_of_the_fictional_weapons() {
    let f = common::fixture(false);
    let engine = f.ctx.require_engine().unwrap();
    let pools = engine.pools().unwrap();
    println!(
        "ranged {} melee {} skipped {:?}",
        pools.ranged.len(),
        pools.melee.len(),
        pools.set.skipped
    );
    assert_eq!(pools.ranged.len(), 15);
    assert_eq!(pools.melee.len(), 8);
    assert!(engine.pool(ItemKind::Ranged).unwrap().roles().len() >= 2);
}
