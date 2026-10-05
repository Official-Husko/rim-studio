//! The optional Combat Extended model comes from its own session, so the vanilla pools never see it.
#![cfg(feature = "tool-designer")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common;

use std::sync::Arc;

use rimstudio_design::model::{CePatchSpec, Sourced};
use rimstudio_ipc_types::designer::{DesignerPreviewRequest, DesignerReferenceListRequest};
use rimstudio_toolkit::designer::{Ctx, preview, reference_list, reference_list_ce};

fn two_session_ctx() -> (common::Fixture, Ctx) {
    let f = common::fixture(true);
    // the main session has no Combat Extended; a second one has it as a reference mod
    let vanilla = common::open_session_with(&f.install, &["ludeon.rimworld", "rs.extra"]);
    let ce = common::open_session(&f.install, true);
    let ctx = f.ctx.with_session(vanilla).with_ce_session(ce);
    (f, ctx)
}

#[test]
fn the_ce_model_is_read_from_the_ce_session_and_vanilla_pools_from_the_main_one() {
    let (_f, ctx) = two_session_ctx();
    let engine = ctx.require_engine().unwrap();
    assert!(engine.ce_available());
    assert_eq!(engine.pools().unwrap().ranged.len(), 15);
    let list = reference_list(
        &ctx,
        DesignerReferenceListRequest {
            limit: 500,
            ..DesignerReferenceListRequest::default()
        },
    )
    .unwrap();
    assert_eq!(list.total, 15);
    assert!(reference_list_ce(&ctx, DesignerReferenceListRequest::default()).is_ok());
}

#[test]
fn without_the_ce_session_a_vanilla_main_session_has_no_combat_extended() {
    let f = common::fixture(false);
    assert!(!f.ctx.require_engine().unwrap().ce_available());
}

#[test]
fn the_ce_readouts_follow_the_opt_in_block_in_the_two_session_setup() {
    let (_f, ctx) = two_session_ctx();
    let mut spec = common::typed_ranged();
    let off = preview(
        &ctx,
        DesignerPreviewRequest {
            draft: common::dto(spec.clone()),
        },
    )
    .unwrap();
    assert!(off.readouts.iter().all(|r| !r.key.starts_with("ce-")));
    spec.ce = Some(CePatchSpec {
        bulk: Some(Sourced::typed(5.5)),
        ..CePatchSpec::default()
    });
    let on = preview(
        &ctx,
        DesignerPreviewRequest {
            draft: common::dto(spec),
        },
    )
    .unwrap();
    let bulk = on.readouts.iter().find(|r| r.key == "ce-bulk").unwrap();
    assert_eq!(bulk.value, Some(5.5));
    assert!(on.diagnostics.iter().all(|d| d.code != "design.ce-absent"));
}

#[test]
fn replacing_a_session_rebuilds_the_engine() {
    let (f, ctx) = two_session_ctx();
    let before = ctx.require_engine().unwrap();
    let again = ctx.require_engine().unwrap();
    assert!(
        Arc::ptr_eq(&before, &again),
        "the engine is cached per snapshot"
    );
    let fewer = ctx.with_session(common::open_session_with(&f.install, &["ludeon.rimworld"]));
    assert_eq!(
        fewer
            .require_engine()
            .unwrap()
            .pools()
            .unwrap()
            .ranged
            .len(),
        14
    );
}
