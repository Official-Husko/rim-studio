//! Applying a fix plan and undoing it: the owner's two weapon mods (fixtures), then the hostile cases.
#![cfg(feature = "tool-project")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

mod common_fix;

use common_fix::*;
use rimstudio_core::jobs::{CancelToken, CollectingProgress, NoopProgress, Progress, ProgressSink};
use rimstudio_ipc_types::project_fix::{LayoutFixItemKindDto as Kind, LayoutFixSelectionDto};
use rimstudio_toolkit::error::ToolkitError;
use rimstudio_toolkit::project::fix_apply::apply;
use rimstudio_toolkit::project::journal::{
    JournalKind, JournalState, JournalStatus, hash_text, journal_path, read_journal, write_journal,
};

fn warnings(w: &World, id: &str) -> usize {
    issues_of(w, id)
        .iter()
        .filter(|i| i.severity != rimstudio_ipc_types::diagnostic::SeverityDto::Info)
        .count()
}

fn files_of(snap: &std::collections::BTreeMap<String, Option<Vec<u8>>>) -> Vec<&str> {
    snap.iter()
        .filter(|(_, v)| v.is_some())
        .map(|(k, _)| k.as_str())
        .collect()
}

#[test]
fn applying_the_gewehr_41_plan_gates_and_moves_the_patch() {
    let w = world();
    let (root, id) = fixture(&w, "gewehr41");
    let before = issues_of(&w, &id);
    assert!(codes(&before).contains(&"layout.ce-outside-gate"));
    let report = apply_all(&w, &id);
    assert_eq!(
        report.done.len(),
        4,
        "{:?} {:?}",
        report.done,
        report.skipped
    );
    assert!(report.skipped.is_empty());
    assert!(!report.cancelled);
    // the patch is where the layout wants it; the old place is gone
    assert!(
        root.join("Compat/CombatExtended/Patches/ce_patch.xml")
            .is_file()
    );
    assert!(!root.join("Patches/ce_patch.xml").exists());
    let lf = std::fs::read_to_string(root.join("LoadFolders.xml")).unwrap();
    assert!(lf.contains("IfModActive=\"ceteam.combatextended\">Compat/CombatExtended</li>"));
    // the layout check, run again by the apply, has fewer findings and no gate finding
    let after = codes(&report.check.issues);
    assert!(!after.contains(&"layout.ce-outside-gate"));
    assert!(!after.contains(&"layout.ce-folder-ungated"));
    assert!(!after.contains(&"layout.missing-folder"));
    assert!(report.check.issues.len() < before.len());
    assert_eq!(report.check.warnings, 0);
    // a second plan has nothing left to do
    let again = plan_of(&w, &id);
    assert!(again.items.is_empty(), "{:?}", again.items);
    assert_eq!(report.edited.len(), 1);
    assert!(report.edited[0].created && report.edited[0].backup_path.is_none());
}

#[test]
fn applying_the_lone_wolf_plan_keeps_the_content_in_common_loaded() {
    let w = world();
    let (root, id) = fixture(&w, "lonewolf");
    let report = apply_all(&w, &id);
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert!(
        root.join("Common/Compat/CombatExtended/Patches/CE_Patch.xml")
            .is_file()
    );
    let lf = std::fs::read_to_string(root.join("LoadFolders.xml")).unwrap();
    let block = lf.split("</v1.3>").next().unwrap();
    assert!(
        block.contains("<li>/</li>") && block.contains("<li>Common</li>"),
        "{lf}"
    );
    assert_eq!(warnings(&w, &id), 0, "{:?}", issues_of(&w, &id));
}

#[test]
fn undo_restores_every_byte_and_every_folder() {
    for name in ["gewehr41", "lonewolf"] {
        let w = world();
        let (root, id) = fixture(&w, name);
        let before = snapshot(&root);
        let report = apply_all(&w, &id);
        assert_ne!(before, snapshot(&root));
        let undone = undo_of(&w, &id, &report.apply_id).unwrap();
        assert_eq!(before, snapshot(&root), "{name}");
        assert_eq!(undone.moved_back.len(), 1);
        assert!(undone.restored.contains(&"LoadFolders.xml".to_owned()));
        assert!(!undone.removed_folders.is_empty());
        // the check is back to what it was
        assert!(codes(&undone.check.issues).contains(&"layout.ce-outside-gate"));
        // a second undo is refused
        assert!(matches!(
            undo_of(&w, &id, &report.apply_id),
            Err(ToolkitError::FixUndoRefused { .. })
        ));
    }
}

#[test]
fn the_history_lists_the_journals_with_their_state() {
    let w = world();
    let (_root, id) = fixture(&w, "gewehr41");
    assert!(history_of(&w, &id).journals.is_empty());
    let report = apply_all(&w, &id);
    let h = history_of(&w, &id);
    assert_eq!(h.journals.len(), 1);
    let j = &h.journals[0];
    assert_eq!(j.apply_id, report.apply_id);
    assert_eq!((j.items_total, j.items_done, j.items_skipped), (4, 4, 0));
    assert!(j.undo_possible && !j.undone && j.undo_blocker.is_none());
    undo_of(&w, &id, &report.apply_id).unwrap();
    let j = &history_of(&w, &id).journals[0];
    assert!(j.undone && !j.undo_possible);
}

#[test]
fn a_backup_of_the_edited_load_folders_is_kept_in_the_data_root() {
    let w = world();
    let lf = "<loadFolders>\n  <v1.6>\n    <li>/</li>\n  </v1.6>\n</loadFolders>\n";
    let (root, id) = mod_at(
        &w,
        "RS_Backup",
        &[("LoadFolders.xml", lf), ("Patches/ce.xml", CE_PATCH)],
    );
    let report = apply_all(&w, &id);
    let edited = &report.edited[0];
    assert!(!edited.created);
    let backup = edited.backup_path.as_deref().unwrap();
    assert!(backup.starts_with(w.roots.data.as_str()));
    assert_eq!(std::fs::read_to_string(backup).unwrap(), lf);
    // every other byte of the file is kept: the entry is added by a span edit
    let now = std::fs::read_to_string(root.join("LoadFolders.xml")).unwrap();
    assert!(now.starts_with("<loadFolders>\n  <v1.6>\n    <li>/</li>\n"));
    assert!(now.contains("IfModActive=\"ceteam.combatextended\">Compat/CombatExtended</li>"));
    undo_of(&w, &id, &report.apply_id).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("LoadFolders.xml")).unwrap(),
        lf
    );
}

#[test]
fn a_legacy_folder_is_renamed_with_the_patches_inside_and_undone() {
    let w = world();
    let lf = "<loadFolders>\n  <v1.6>\n    <li>/</li>\n    <li IfModActive=\"ceteam.combatextended\">CE</li>\n  </v1.6>\n</loadFolders>\n";
    let (root, id) = mod_at(
        &w,
        "RS_Legacy",
        &[
            ("LoadFolders.xml", lf),
            ("CE/Patches/a.xml", CE_PATCH),
            ("Patches/b.xml", CE_PATCH),
        ],
    );
    let before = snapshot(&root);
    let p = plan_of(&w, &id);
    // the stray patch goes into the existing (legacy) folder, then the folder is renamed
    let stray = p.items.iter().find(|i| i.from == "Patches/b.xml").unwrap();
    assert_eq!(stray.to, "CE/Patches/b.xml");
    let report = apply_selected(&w, &id, &p, select_all(&p)).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert!(root.join("Compat/CombatExtended/Patches/a.xml").is_file());
    assert!(root.join("Compat/CombatExtended/Patches/b.xml").is_file());
    assert!(!root.join("CE").exists());
    let now = std::fs::read_to_string(root.join("LoadFolders.xml")).unwrap();
    assert!(now.contains(">Compat/CombatExtended</li>") && !now.contains(">CE</li>"));
    assert!(
        report
            .check
            .issues
            .iter()
            .all(|i| i.code != "layout.ce-legacy-folder"),
        "{:?}",
        report.check.issues
    );
    undo_of(&w, &id, &report.apply_id).unwrap();
    assert_eq!(before, snapshot(&root));
}

#[test]
fn a_folder_with_a_wrong_letter_case_is_corrected_and_restored() {
    let w = world();
    let (root, id) = mod_at(
        &w,
        "RS_Case",
        &[(
            "defs/ThingDefs_Misc/Weapons/RangedIndustrial/RS_R.xml",
            &rifle("RS_R", "Things/R"),
        )],
    );
    let before = snapshot(&root);
    let p = plan_of(&w, &id);
    let case = p
        .items
        .iter()
        .find(|i| i.issue_code == "layout.folder-case")
        .unwrap();
    assert_eq!((case.from.as_str(), case.to.as_str()), ("defs", "Defs"));
    let report = apply_selected(
        &w,
        &id,
        &p,
        vec![LayoutFixSelectionDto {
            id: case.id.clone(),
            rename_on_conflict: false,
        }],
    )
    .unwrap();
    assert_eq!(report.done.len(), 1, "{:?}", report.skipped);
    let names: Vec<String> = std::fs::read_dir(root.as_std_path())
        .unwrap()
        .flatten()
        .map(|e| e.file_name().into_string().unwrap())
        .collect();
    assert!(names.contains(&"Defs".to_owned()) && !names.contains(&"defs".to_owned()));
    undo_of(&w, &id, &report.apply_id).unwrap();
    assert_eq!(before, snapshot(&root));
}

#[test]
fn a_destination_that_exists_is_never_overwritten() {
    let w = world();
    let (root, id) = mod_at(
        &w,
        "RS_Conflict",
        &[
            ("Patches/ce.xml", CE_PATCH),
            ("Compat/CombatExtended/Patches/ce.xml", "<Patch/>"),
        ],
    );
    let p = plan_of(&w, &id);
    let report = apply_selected(&w, &id, &p, select_all(&p)).unwrap();
    // the move is refused, the gate (a separate item) is applied alone? no: the move needs the gate, not the reverse
    let moved = p.items.iter().find(|i| i.kind == Kind::MoveFile).unwrap();
    assert!(
        report
            .skipped
            .iter()
            .any(|s| s.id == moved.id && s.reason.contains("exists")),
        "{:?}",
        report.skipped
    );
    assert_eq!(
        std::fs::read_to_string(root.join("Compat/CombatExtended/Patches/ce.xml")).unwrap(),
        "<Patch/>"
    );
    assert_eq!(
        std::fs::read_to_string(root.join("Patches/ce.xml")).unwrap(),
        CE_PATCH
    );
    // with the numbered name accepted the file moves next to it
    let again = plan_of(&w, &id);
    let moved = again
        .items
        .iter()
        .find(|i| i.kind == Kind::MoveFile)
        .unwrap();
    let sel: Vec<LayoutFixSelectionDto> = again
        .items
        .iter()
        .filter(|i| i.applicable)
        .map(|i| LayoutFixSelectionDto {
            id: i.id.clone(),
            rename_on_conflict: i.id == moved.id,
        })
        .collect();
    let report = apply_selected(&w, &id, &again, sel).unwrap();
    assert!(report.skipped.is_empty(), "{:?}", report.skipped);
    assert_eq!(
        std::fs::read_to_string(root.join("Compat/CombatExtended/Patches/ce_2.xml")).unwrap(),
        CE_PATCH
    );
    assert_eq!(
        std::fs::read_to_string(root.join("Compat/CombatExtended/Patches/ce.xml")).unwrap(),
        "<Patch/>"
    );
}

#[test]
fn a_file_changed_between_the_plan_and_the_apply_refuses_the_apply() {
    let w = world();
    let (root, id) = fixture(&w, "gewehr41");
    let p = plan_of(&w, &id);
    let before = snapshot(&root);
    // another program edits the patch after the plan was reviewed
    let path = root.join("Patches/ce_patch.xml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("\n<!-- edited -->\n");
    std::fs::write(&path, &text).unwrap();
    let changed = snapshot(&root);
    assert_ne!(before, changed);
    let err = apply_selected(&w, &id, &p, select_all(&p)).unwrap_err();
    assert!(matches!(err, ToolkitError::PlanStale { .. }), "{err}");
    assert_eq!(err.code(), "designer.plan-stale");
    assert_eq!(changed, snapshot(&root));
    assert!(history_of(&w, &id).journals.is_empty());
}

#[test]
fn a_plan_id_that_is_not_the_current_one_refuses_the_apply() {
    let w = world();
    let (root, id) = fixture(&w, "gewehr41");
    let before = snapshot(&root);
    let mut p = plan_of(&w, &id);
    p.plan_id = "0".repeat(32);
    let err = apply_selected(&w, &id, &p, select_all(&p)).unwrap_err();
    assert!(matches!(err, ToolkitError::PlanStale { .. }));
    assert_eq!(before, snapshot(&root));
}

#[test]
fn items_that_need_review_unknown_ids_and_orphaned_items_are_skipped_with_reasons() {
    let w = world();
    let mixed = "<Patch><Operation Class=\"PatchOperationAdd\"><xpath>/Defs/ThingDef</xpath><value><x/></value></Operation>\
<Operation Class=\"PatchOperationAdd\"><xpath>/Defs/ThingDef</xpath><value><li Class=\"CombatExtended.ToolCE\"/></value></Operation></Patch>";
    let (root, id) = mod_at(
        &w,
        "RS_Skip",
        &[("Patches/mixed.xml", mixed), ("Patches/ce.xml", CE_PATCH)],
    );
    let before = snapshot(&root);
    let p = plan_of(&w, &id);
    let review = p
        .items
        .iter()
        .find(|i| i.from == "Patches/mixed.xml")
        .unwrap();
    let ce = p.items.iter().find(|i| i.from == "Patches/ce.xml").unwrap();
    let sel = |i: &str| LayoutFixSelectionDto {
        id: i.to_owned(),
        rename_on_conflict: false,
    };
    // the move without its gate item, a review item, an unknown id
    let report = apply_selected(
        &w,
        &id,
        &p,
        vec![sel(&review.id), sel(&ce.id), sel("fix-999-nope")],
    )
    .unwrap();
    assert!(report.done.is_empty());
    assert_eq!(report.skipped.len(), 3);
    let reason = |i: &str| {
        report
            .skipped
            .iter()
            .find(|s| s.id == i)
            .unwrap()
            .reason
            .clone()
    };
    assert!(reason(&review.id).contains("do not use Combat Extended"));
    assert!(reason(&ce.id).contains("applied together with"));
    assert!(reason("fix-999-nope").contains("not in the plan"));
    assert_eq!(before, snapshot(&root));
    // nothing was changed, so no journal needs to exist
    assert!(history_of(&w, &id).journals.is_empty());
}

#[cfg(unix)]
#[test]
fn a_link_in_the_destination_path_is_refused_and_nothing_leaves_the_project() {
    let w = world();
    let outside = w.base.join("outside");
    std::fs::create_dir_all(outside.as_std_path()).unwrap();
    std::fs::write(outside.join("keep.txt").as_std_path(), "keep").unwrap();
    let (root, id) = mod_at(&w, "RS_Link", &[("Patches/ce.xml", CE_PATCH)]);
    std::os::unix::fs::symlink(outside.as_std_path(), root.join("Compat").as_std_path()).unwrap();
    let out_before = snapshot(&outside);
    let p = plan_of(&w, &id);
    let m = p.items.iter().find(|i| i.from == "Patches/ce.xml").unwrap();
    assert!(!m.applicable, "{m:?}");
    assert!(
        m.review_reason.as_deref().unwrap().contains("refused"),
        "{m:?}"
    );
    let report = apply_selected(&w, &id, &p, select_all(&p)).unwrap();
    assert!(report.done.iter().all(|d| d.kind != Kind::MoveFile));
    assert_eq!(out_before, snapshot(&outside));
    assert!(root.join("Patches/ce.xml").is_file());
}

#[cfg(unix)]
#[test]
fn a_linked_source_file_is_not_a_candidate() {
    let w = world();
    let outside = w.base.join("outside");
    std::fs::create_dir_all(outside.as_std_path()).unwrap();
    std::fs::write(outside.join("ce.xml").as_std_path(), CE_PATCH).unwrap();
    let (root, id) = mod_at(&w, "RS_LinkSrc", &[]);
    std::fs::create_dir_all(root.join("Patches").as_std_path()).unwrap();
    std::os::unix::fs::symlink(
        outside.join("ce.xml").as_std_path(),
        root.join("Patches/ce.xml").as_std_path(),
    )
    .unwrap();
    let p = plan_of(&w, &id);
    assert!(
        p.items.iter().all(|i| i.from != "Patches/ce.xml"),
        "{:?}",
        p.items
    );
    assert_eq!(
        std::fs::read_to_string(outside.join("ce.xml")).unwrap(),
        CE_PATCH
    );
}

/// The state after an interruption at every point leaves a journal that the undo can use.
#[test]
fn a_crash_between_items_leaves_a_consistent_journal_that_undoes_cleanly() {
    let w0 = world();
    let (_, id0) = fixture(&w0, "gewehr41");
    let total = plan_of(&w0, &id0).items.len();
    for stop in 0..total {
        let w = world();
        let (root, id) = fixture(&w, "gewehr41");
        let before = snapshot(&root);
        let p = plan_of(&w, &id);
        // the run stops right after item `stop` changed the disk, before the journal recorded it
        let err = apply_hooked(&w, &id, &p, &|n| n != stop).unwrap_err();
        assert!(matches!(err, ToolkitError::Internal { .. }), "{err}");
        let h = history_of(&w, &id);
        assert_eq!(h.journals.len(), 1, "stop {stop}");
        let j = &h.journals[0];
        assert!(j.undo_possible, "stop {stop}: {:?}", j.undo_blocker);
        let journal = read_journal(&w.env, &id, &j.apply_id).unwrap();
        assert_eq!(journal.state, JournalState::Started);
        assert_eq!(journal.items.len(), total);
        // the item in flight is still listed as pending; the earlier ones are done
        assert_eq!(journal.items[stop].status, JournalStatus::Pending);
        assert!(
            journal.items[..stop]
                .iter()
                .all(|i| i.status == JournalStatus::Done)
        );
        undo_of(&w, &id, &j.apply_id).unwrap();
        assert_eq!(before, snapshot(&root), "stop {stop}");
    }
}

#[test]
fn a_cancelled_job_keeps_what_was_done_and_it_can_be_undone() {
    struct CancelOnFirst(CancelToken, std::sync::atomic::AtomicBool);
    impl ProgressSink for CancelOnFirst {
        fn report(&self, _: Progress) {
            if !self.1.swap(true, std::sync::atomic::Ordering::SeqCst) {
                self.0.cancel();
            }
        }
    }
    let w = world();
    let (root, id) = fixture(&w, "gewehr41");
    let before = snapshot(&root);
    let p = plan_of(&w, &id);
    let cancel = CancelToken::new();
    let sink = CancelOnFirst(cancel.clone(), std::sync::atomic::AtomicBool::new(false));
    let report = apply(
        &w.env,
        &apply_request(&id, &p, select_all(&p)),
        &[],
        &sink,
        &cancel,
    )
    .unwrap();
    assert!(report.cancelled);
    assert_eq!(report.done.len(), 1);
    undo_of(&w, &id, &report.apply_id).unwrap();
    assert_eq!(before, snapshot(&root));
    // cancelled before anything: no journal, nothing changed
    let p = plan_of(&w, &id);
    let cancelled = CancelToken::new();
    cancelled.cancel();
    let err = apply(
        &w.env,
        &apply_request(&id, &p, select_all(&p)),
        &[],
        &NoopProgress,
        &cancelled,
    )
    .unwrap_err();
    assert!(matches!(err, ToolkitError::Cancelled));
    assert_eq!(before, snapshot(&root));
    let progress = CollectingProgress::new();
    let report = apply(
        &w.env,
        &apply_request(&id, &p, select_all(&p)),
        &[],
        &progress,
        &CancelToken::new(),
    )
    .unwrap();
    assert_eq!(progress.len(), report.done.len());
}

// ---- journals that are not what the apply wrote ----

fn applied_gewehr() -> (World, camino::Utf8PathBuf, String, String) {
    let w = world();
    let (root, id) = fixture(&w, "gewehr41");
    let report = apply_all(&w, &id);
    (w, root, id, report.apply_id)
}

#[test]
fn a_journal_with_a_changed_byte_is_refused_and_nothing_changes() {
    let (w, root, id, apply_id) = applied_gewehr();
    let after = snapshot(&root);
    let path = journal_path(&w.env, &id, &apply_id);
    let text = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, text.replace("ce_patch.xml", "ce_patch.xmx")).unwrap();
    let err = undo_of(&w, &id, &apply_id).unwrap_err();
    assert!(
        matches!(err, ToolkitError::FixJournalDamaged { .. }),
        "{err}"
    );
    assert_eq!(err.code(), "project.fix-journal-damaged");
    assert_eq!(after, snapshot(&root));
    let h = history_of(&w, &id);
    assert!(!h.journals[0].undo_possible);
    // a file that is not JSON, and an unknown id
    std::fs::write(&path, "not json").unwrap();
    assert!(matches!(
        undo_of(&w, &id, &apply_id),
        Err(ToolkitError::FixJournalDamaged { .. })
    ));
    assert!(matches!(
        undo_of(&w, &id, "fix-1-aaaaaaaa"),
        Err(ToolkitError::FixNotFound { .. })
    ));
    assert!(matches!(
        undo_of(&w, &id, "../../x"),
        Err(ToolkitError::FixNotFound { .. })
    ));
}

fn forge(
    w: &World,
    id: &str,
    apply_id: &str,
    edit: impl FnOnce(&mut rimstudio_toolkit::project::journal::Journal),
) {
    // an attacker who can write the data folder also fixes the checksum
    let mut j = read_journal(&w.env, id, apply_id).unwrap();
    edit(&mut j);
    write_journal(&w.env, &j).unwrap();
}

#[test]
fn a_forged_journal_cannot_move_content_outside_the_project_or_elsewhere() {
    let (w, root, id, apply_id) = applied_gewehr();
    let after = snapshot(&root);
    let secret = w.base.join("secret.txt");
    std::fs::write(secret.as_std_path(), "secret").unwrap();
    // the move back is pointed out of the project
    forge(&w, &id, &apply_id, |j| {
        let m = j
            .items
            .iter_mut()
            .find(|i| i.kind == JournalKind::MoveFile)
            .unwrap();
        m.from = "../secret.txt".to_owned();
    });
    let err = undo_of(&w, &id, &apply_id).unwrap_err();
    assert!(
        matches!(err, ToolkitError::FixJournalDamaged { .. }),
        "{err}"
    );
    assert_eq!(after, snapshot(&root));
    assert_eq!(
        std::fs::read_to_string(secret.as_std_path()).unwrap(),
        "secret"
    );
}

#[test]
fn a_forged_journal_with_another_destination_is_refused_by_the_hash() {
    let (w, root, id, apply_id) = applied_gewehr();
    let after = snapshot(&root);
    // the journal claims the About file is what was moved
    forge(&w, &id, &apply_id, |j| {
        let m = j
            .items
            .iter_mut()
            .find(|i| i.kind == JournalKind::MoveFile)
            .unwrap();
        m.to = "About/About.xml".to_owned();
        m.from = "About/Moved.xml".to_owned();
    });
    let err = undo_of(&w, &id, &apply_id).unwrap_err();
    assert!(matches!(err, ToolkitError::FixUndoRefused { .. }), "{err}");
    assert_eq!(after, snapshot(&root));
}

#[test]
fn a_forged_journal_cannot_write_a_file_other_than_load_folders() {
    let (w, root, id, apply_id) = applied_gewehr();
    let after = snapshot(&root);
    forge(&w, &id, &apply_id, |j| {
        let e = j
            .items
            .iter_mut()
            .find(|i| i.kind == JournalKind::CreateLoadFolders)
            .unwrap();
        e.to = "About/About.xml".to_owned();
    });
    let err = undo_of(&w, &id, &apply_id).unwrap_err();
    assert!(
        matches!(err, ToolkitError::FixJournalDamaged { .. }),
        "{err}"
    );
    assert_eq!(after, snapshot(&root));
}

#[test]
fn a_journal_of_another_project_is_refused() {
    let (w, root, id, apply_id) = applied_gewehr();
    let after = snapshot(&root);
    forge(&w, &id, &apply_id, |j| {
        j.project_root = "/somewhere/else".to_owned()
    });
    let err = undo_of(&w, &id, &apply_id).unwrap_err();
    assert!(
        matches!(err, ToolkitError::FixJournalDamaged { .. }),
        "{err}"
    );
    assert_eq!(after, snapshot(&root));
}

#[test]
fn an_undo_is_refused_when_the_moved_file_was_changed_afterwards() {
    let (w, root, id, apply_id) = applied_gewehr();
    let moved = root.join("Compat/CombatExtended/Patches/ce_patch.xml");
    std::fs::write(&moved, "<Patch><!-- mine --></Patch>").unwrap();
    let after = snapshot(&root);
    let err = undo_of(&w, &id, &apply_id).unwrap_err();
    assert!(matches!(err, ToolkitError::FixUndoRefused { .. }), "{err}");
    assert!(
        err.to_string()
            .contains("Compat/CombatExtended/Patches/ce_patch.xml"),
        "{err}"
    );
    assert_eq!(after, snapshot(&root));
    let j = &history_of(&w, &id).journals[0];
    assert!(!j.undo_possible);
    assert!(
        j.undo_blocker
            .as_deref()
            .unwrap()
            .contains("changed after the fix")
    );
}

#[test]
fn an_undo_is_refused_when_the_original_place_is_taken_or_the_file_is_gone() {
    let (w, root, id, apply_id) = applied_gewehr();
    std::fs::write(root.join("Patches/ce_patch.xml"), "new file").unwrap();
    let after = snapshot(&root);
    let err = undo_of(&w, &id, &apply_id).unwrap_err();
    assert!(matches!(err, ToolkitError::FixUndoRefused { .. }), "{err}");
    assert!(err.to_string().contains("original place"), "{err}");
    assert_eq!(after, snapshot(&root));
    std::fs::remove_file(root.join("Patches/ce_patch.xml")).unwrap();
    std::fs::remove_file(root.join("Compat/CombatExtended/Patches/ce_patch.xml")).unwrap();
    let err = undo_of(&w, &id, &apply_id).unwrap_err();
    assert!(err.to_string().contains("no longer there"), "{err}");
}

#[test]
fn an_undo_is_refused_when_load_folders_was_edited_again() {
    let (w, root, id, apply_id) = applied_gewehr();
    let path = root.join("LoadFolders.xml");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str("<!-- later -->\n");
    std::fs::write(&path, text).unwrap();
    let after = snapshot(&root);
    let err = undo_of(&w, &id, &apply_id).unwrap_err();
    assert!(matches!(err, ToolkitError::FixUndoRefused { .. }), "{err}");
    assert!(err.to_string().contains("LoadFolders.xml"));
    assert_eq!(after, snapshot(&root));
}

#[test]
fn files_added_later_to_a_created_folder_keep_the_folder() {
    let (w, root, id, apply_id) = applied_gewehr();
    std::fs::write(root.join("Sounds/mine.wav"), "wav").unwrap();
    let undone = undo_of(&w, &id, &apply_id).unwrap();
    assert!(root.join("Sounds/mine.wav").is_file());
    assert!(!undone.removed_folders.contains(&"Sounds".to_owned()));
    assert!(!root.join("Compat").exists());
    let _ = files_of(&snapshot(&root));
    let _ = hash_text("x");
}

fn legacy_mod(w: &World) -> (camino::Utf8PathBuf, String) {
    let lf = "<loadFolders>\n  <v1.6>\n    <li>/</li>\n    <li IfModActive=\"ceteam.combatextended\">CE</li>\n  </v1.6>\n</loadFolders>\n";
    mod_at(
        w,
        "RS_LegacyCrash",
        &[
            ("LoadFolders.xml", lf),
            ("CE/Patches/a.xml", CE_PATCH),
            ("Patches/b.xml", CE_PATCH),
        ],
    )
}

#[test]
fn a_crash_at_any_point_of_a_rename_with_an_entry_edit_can_be_undone() {
    let w0 = world();
    let (_, id0) = legacy_mod(&w0);
    let total = plan_of(&w0, &id0).items.len();
    assert!(total >= 4, "{total}");
    for stop in 0..total {
        let w = world();
        let (root, id) = legacy_mod(&w);
        let before = snapshot(&root);
        let p = plan_of(&w, &id);
        let err = apply_hooked(&w, &id, &p, &|n| n != stop).unwrap_err();
        assert!(matches!(err, ToolkitError::Internal { .. }), "{err}");
        let h = history_of(&w, &id);
        assert!(
            h.journals[0].undo_possible,
            "stop {stop}: {:?}",
            h.journals[0].undo_blocker
        );
        undo_of(&w, &id, &h.journals[0].apply_id).unwrap();
        assert_eq!(before, snapshot(&root), "stop {stop}");
    }
}

#[test]
fn a_legacy_folder_next_to_the_standard_one_is_left_alone() {
    let w = world();
    let (root, id) = mod_at(
        &w,
        "RS_TwoCe",
        &[
            ("CE/Patches/a.xml", CE_PATCH),
            ("Compat/CombatExtended/Patches/b.xml", CE_PATCH),
        ],
    );
    let p = plan_of(&w, &id);
    // the standard folder is the one the layout uses; nothing moves the other one onto it
    assert!(
        p.items.iter().all(|i| i.kind != Kind::MoveFolder),
        "{:?}",
        p.items
    );
    let before = snapshot(&root);
    apply_selected(&w, &id, &p, select_all(&p)).unwrap();
    assert!(root.join("CE/Patches/a.xml").is_file());
    let _ = before;
}

#[test]
fn a_gate_that_cannot_be_written_keeps_the_patches_where_they_are() {
    let w = world();
    let lf = "<loadFolders>\n  <v1.6>\n    <li>/</li>\n  </v1.6>\n</loadFolders>\n";
    let (root, id) = mod_at(
        &w,
        "RS_ReadOnly",
        &[("LoadFolders.xml", lf), ("Patches/ce.xml", CE_PATCH)],
    );
    let path = root.join("LoadFolders.xml");
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    perms.set_readonly(true);
    std::fs::set_permissions(&path, perms.clone()).unwrap();
    let p = plan_of(&w, &id);
    let report = apply_selected(&w, &id, &p, select_all(&p)).unwrap();
    #[allow(clippy::permissions_set_readonly_false)]
    {
        perms.set_readonly(false);
        std::fs::set_permissions(&path, perms).unwrap();
    }
    assert!(
        report.done.iter().all(|d| d.kind != Kind::MoveFile),
        "{:?}",
        report.done
    );
    assert!(
        report
            .skipped
            .iter()
            .any(|s| s.reason.contains("read only")),
        "{:?}",
        report.skipped
    );
    assert!(
        report
            .skipped
            .iter()
            .any(|s| s.reason.contains("was not applied")),
        "{:?}",
        report.skipped
    );
    assert!(root.join("Patches/ce.xml").is_file());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), lf);
}

#[test]
fn the_journal_lives_in_the_data_root_and_never_in_the_mod() {
    let (w, root, _id, apply_id) = applied_gewehr();
    let journals = w.roots.data.join("project-fix-journal");
    let found: Vec<_> = walkdir_names(journals.as_std_path());
    assert!(
        found.iter().any(|n| n == &format!("{apply_id}.json")),
        "{found:?}"
    );
    let in_mod = snapshot(&root);
    assert!(
        in_mod
            .keys()
            .all(|k| !k.contains("journal") && !k.ends_with(".json")),
        "{:?}",
        in_mod.keys()
    );
}

fn walkdir_names(dir: &std::path::Path) -> Vec<String> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        if e.file_type().unwrap().is_dir() {
            out.extend(walkdir_names(&e.path()));
        } else {
            out.push(e.file_name().into_string().unwrap());
        }
    }
    out
}
