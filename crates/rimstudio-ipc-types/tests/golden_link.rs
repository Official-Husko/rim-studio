//! Golden JSON of the link DTOs. Sample values are invented (prefix `RS_`).
//! Run with `RIMSTUDIO_UPDATE_GOLDEN=1` to rewrite the files, then review the diff.

use std::path::PathBuf;

use rimstudio_ipc_types::diagnostic::{DiagnosticDto, SeverityDto};
use rimstudio_ipc_types::project_link::*;
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

fn linked() -> ProjectLinkStatusDto {
    ProjectLinkStatusDto {
        project_id: "p-1a2b3c4d".into(),
        package_id: Some("rs.fiction.arms".into()),
        game_found: true,
        game_folder: Some("/games/RimWorld".into()),
        mods_folder: Some("/games/RimWorld/Mods".into()),
        mods_exists: true,
        mods_read_only: false,
        link_name: Some("RS_Arms".into()),
        entry_path: Some("/games/RimWorld/Mods/RS_Arms".into()),
        state: ProjectLinkStateDto::Linked,
        mode: Some(ProjectLinkModeDto::Symlink),
        points_to: Some("/mods/RS_Arms".into()),
        game_running: GameRunningDto::NotRunning,
        support: LinkSupportDto {
            symlink: true,
            junction: false,
            needs_privilege: false,
        },
        can_create: false,
        can_remove: true,
        manual_command: Some("ln -s '/mods/RS_Arms' '/games/RimWorld/Mods/RS_Arms'".into()),
        active_in_game: ActiveInGameDto::Inactive,
        has_ce_patch: true,
        ce_active_in_game: ActiveInGameDto::Unknown,
        diagnostics: Vec::new(),
    }
}

fn blocked() -> ProjectLinkStatusDto {
    ProjectLinkStatusDto {
        state: ProjectLinkStateDto::ForeignFolder,
        mode: None,
        points_to: None,
        game_running: GameRunningDto::Running,
        can_create: false,
        can_remove: false,
        diagnostics: vec![DiagnosticDto {
            code: "deploy.name-taken".into(),
            severity: SeverityDto::Warning,
            message: "something with this name already exists in the Mods folder".into(),
            field: None,
            mod_idx: None,
            file_id: None,
            span: None,
            args: Default::default(),
        }],
        ..linked()
    }
}

#[test]
fn golden_link_requests() {
    check(
        "link-requests",
        &serde_json::json!({
            "status": ProjectLinkStatusRequest { project_id: "p-1a2b3c4d".into() },
            "create": ProjectLinkCreateRequest {
                project_id: "p-1a2b3c4d".into(),
                mode: ProjectLinkModeDto::Junction,
                confirm_game_running: true,
            },
            "createDefaults": ProjectLinkCreateRequest::default(),
            "remove": ProjectLinkRemoveRequest { project_id: "p-1a2b3c4d".into() },
        }),
    );
}

#[test]
fn golden_link_status_linked() {
    check("link-status-linked", &linked());
}

#[test]
fn golden_link_result_refused() {
    check(
        "link-result-refused",
        &ProjectLinkResultDto {
            done: false,
            refusal: Some(ProjectLinkRefusalDto {
                code: "deploy.name-taken".into(),
                message: "something with this name already exists in the Mods folder".into(),
            }),
            status: blocked(),
        },
    );
}

#[test]
fn golden_link_result_done() {
    check(
        "link-result-done",
        &ProjectLinkResultDto {
            done: true,
            refusal: None,
            status: linked(),
        },
    );
}

#[test]
fn a_create_request_without_a_mode_is_a_symlink() {
    let req: ProjectLinkCreateRequest =
        serde_json::from_str(r#"{"projectId":"p-1"}"#).unwrap_or_default();
    assert_eq!(req.mode, ProjectLinkModeDto::Symlink);
    assert!(!req.confirm_game_running);
}
