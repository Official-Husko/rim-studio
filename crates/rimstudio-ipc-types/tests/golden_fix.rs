//! Golden JSON of the layout fix DTOs (plan, apply, undo, history). Sample values are invented (prefix `RS_`).
//! Run with `RIMSTUDIO_UPDATE_GOLDEN=1` to rewrite the files, then review the diff.

use std::path::PathBuf;

use rimstudio_ipc_types::project::{LayoutProfileDto, ProjectLayoutCheckDto};
use rimstudio_ipc_types::project_fix::*;
use serde::Serialize;

fn check<T: Serialize>(name: &str, value: &T) {
    let mut text = serde_json::to_string_pretty(value).unwrap_or_default();
    text.push('\n');
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("golden")
        .join(format!("{name}.json"));
    if std::env::var_os("RIMSTUDIO_UPDATE_GOLDEN").is_some() {
        if let Err(e) = std::fs::write(&path, &text) {
            panic!("cannot write {}: {e}", path.display());
        }
        return;
    }
    let expected = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "missing golden file {} ({e}); run with RIMSTUDIO_UPDATE_GOLDEN=1",
            path.display()
        )
    });
    assert_eq!(
        text.replace("\r\n", "\n"),
        expected.replace("\r\n", "\n"),
        "golden mismatch for {name}"
    );
}

fn plan() -> ProjectLayoutFixPlanDto {
    let gate = LayoutFixItemDto {
        id: "fix-003-385f9f".into(),
        kind: LayoutFixItemKindDto::CreateLoadFolders,
        issue_code: "layout.ce-outside-gate".into(),
        from: String::new(),
        to: "LoadFolders.xml".into(),
        why: "A new LoadFolders.xml lists Compat/CombatExtended so that it loads only when Combat Extended is active.".into(),
        risk: LayoutFixRiskDto::Safe,
        applicable: true,
        review_reason: None,
        diff: Some("--- a/LoadFolders.xml\n+++ b/LoadFolders.xml\n@@ -0,0 +1,3 @@\n+<loadFolders>\n+  <v1.6>\n+  </v1.6>\n".into()),
        conflict: None,
        references: vec![],
        requires: vec![],
    };
    let moved = LayoutFixItemDto {
        id: "fix-004-0c324d".into(),
        kind: LayoutFixItemKindDto::MoveFile,
        issue_code: "layout.ce-outside-gate".into(),
        from: "Patches/RS_ce_patch.xml".into(),
        to: "Compat/CombatExtended/Patches/RS_ce_patch.xml".into(),
        why: "The file moves into the gated folder.".into(),
        risk: LayoutFixRiskDto::Safe,
        applicable: true,
        review_reason: None,
        diff: None,
        conflict: Some(LayoutFixConflictDto {
            destination_exists: true,
            suggested_to: Some("Compat/CombatExtended/Patches/RS_ce_patch_2.xml".into()),
        }),
        references: vec![],
        requires: vec!["fix-003-385f9f".into()],
    };
    let review = LayoutFixItemDto {
        id: "fix-005-aa11bb".into(),
        kind: LayoutFixItemKindDto::MoveFolder,
        issue_code: "layout.folder-case".into(),
        from: "textures".into(),
        to: "Textures".into(),
        why: "The folder is named textures.".into(),
        risk: LayoutFixRiskDto::NeedsReview,
        applicable: false,
        review_reason: Some("textures and sounds are referenced by path from definitions".into()),
        diff: None,
        conflict: None,
        references: vec![LayoutFixReferenceDto {
            path: "Defs/RS_Gun.xml".into(),
            line: 12,
            text: "<texPath>Things/RS_Gun</texPath>".into(),
        }],
        requires: vec![],
    };
    ProjectLayoutFixPlanDto {
        project_id: "p-1a2b3c4d".into(),
        plan_id: "ebac9a61a265145f94c35aba8f796ea9".into(),
        items: vec![gate, moved, review],
        safe: 2,
        needs_review: 1,
        conflicts: 1,
    }
}

fn check_dto() -> ProjectLayoutCheckDto {
    ProjectLayoutCheckDto {
        project_id: "p-1a2b3c4d".into(),
        profile: LayoutProfileDto::CoreStyle,
        issues: vec![],
        errors: 0,
        warnings: 0,
        infos: 0,
        auto_fixable: 0,
    }
}

#[test]
fn golden_fix_plan() {
    check("fix-plan", &plan());
}

#[test]
fn golden_fix_apply() {
    check(
        "fix-apply",
        &ProjectLayoutFixApplyDto {
            project_id: "p-1a2b3c4d".into(),
            apply_id: "fix-1767225600000-1a2b3c4d".into(),
            plan_id: "ebac9a61a265145f94c35aba8f796ea9".into(),
            done: vec![LayoutFixDoneDto {
                id: "fix-004-0c324d".into(),
                kind: LayoutFixItemKindDto::MoveFile,
                from: "Patches/RS_ce_patch.xml".into(),
                to: "Compat/CombatExtended/Patches/RS_ce_patch.xml".into(),
                copied: false,
            }],
            edited: vec![LayoutFixEditedDto {
                path: "LoadFolders.xml".into(),
                created: false,
                backup_path: Some(
                    "/data/project-backups/p-1a2b3c4d/root/LoadFolders.xml.2026-10-05".into(),
                ),
            }],
            skipped: vec![LayoutFixSkippedDto {
                id: "fix-005-aa11bb".into(),
                reason: "textures and sounds are referenced by path from definitions".into(),
            }],
            cancelled: false,
            check: check_dto(),
        },
    );
}

#[test]
fn golden_fix_undo() {
    check(
        "fix-undo",
        &ProjectLayoutFixUndoDto {
            project_id: "p-1a2b3c4d".into(),
            apply_id: "fix-1767225600000-1a2b3c4d".into(),
            moved_back: vec!["Patches/RS_ce_patch.xml".into()],
            restored: vec!["LoadFolders.xml".into()],
            removed_folders: vec![
                "Compat/CombatExtended/Patches".into(),
                "Compat/CombatExtended".into(),
                "Compat".into(),
            ],
            check: check_dto(),
        },
    );
}

#[test]
fn golden_fix_history() {
    check(
        "fix-history",
        &ProjectLayoutFixHistoryDto {
            project_id: "p-1a2b3c4d".into(),
            journals: vec![
                LayoutFixJournalDto {
                    apply_id: "fix-1767225600000-1a2b3c4d".into(),
                    plan_id: "ebac9a61a265145f94c35aba8f796ea9".into(),
                    created_ms: 1_767_225_600_000,
                    items_total: 3,
                    items_done: 2,
                    items_skipped: 1,
                    undone: false,
                    undo_possible: true,
                    undo_blocker: None,
                },
                LayoutFixJournalDto {
                    apply_id: "fix-1767139200000-0f0f0f0f".into(),
                    plan_id: "00000000000000000000000000000000".into(),
                    created_ms: 1_767_139_200_000,
                    items_total: 1,
                    items_done: 1,
                    items_skipped: 0,
                    undone: false,
                    undo_possible: false,
                    undo_blocker: Some("the layout fix cannot be undone: Compat/CombatExtended/Patches/RS_ce_patch.xml was changed after the fix".into()),
                },
            ],
        },
    );
}
