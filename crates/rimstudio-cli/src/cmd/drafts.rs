//! Drafts: loading a draft by id or file, saving it back, and the `drafts` commands.

use serde_json::{Value, json};

use crate::cli::DraftsCmd;
use crate::cmd::project;
use crate::draftops::apply_sets;
use crate::error::{CliError, CliResult};
use crate::fmt::{Table, arr_at, bool_at, str_at, u64_at};
use crate::session::Session;

/// A draft read from the store or a file.
#[derive(Debug, Clone)]
pub(crate) struct Loaded {
    /// The draft JSON (`DraftDto`).
    pub(crate) draft: Value,
    /// The draft id when it came from the store.
    pub(crate) id: Option<String>,
}

/// The id of the project of a summary.
#[must_use]
pub(crate) fn project_id(summary: &Value) -> String {
    str_at(summary, "/projectId").to_owned()
}

/// The drafts of a project, newest first.
///
/// # Errors
/// The envelope of `designer_draft_list`.
pub(crate) fn list(s: &Session, project_id: &str) -> CliResult<Vec<Value>> {
    let reply = s.call("designer_draft_list", json!({"projectId": project_id}))?;
    Ok(arr_at(&reply.value, "/drafts").to_vec())
}

fn draft_of_file(text: &str, file: &str) -> CliResult<Value> {
    let v: Value = serde_json::from_str(text)
        .map_err(|e| CliError::usage(format!("{file} is not valid JSON: {e}")))?;
    let draft = match v.get("draft") {
        Some(d) if d.is_object() => d.clone(),
        _ => v,
    };
    if draft.get("spec").is_none() {
        return Err(CliError::usage(format!(
            "{file} is not a draft (it has no spec)"
        )));
    }
    Ok(draft)
}

/// Loads a draft: an id in the project's store, or the path of a draft file.
///
/// # Errors
/// A usage error when neither a project nor a readable file is given, a failure when the id is unknown.
pub(crate) fn load(s: &Session, reference: &str, summary: Option<&Value>) -> CliResult<Loaded> {
    if let Some(summary) = summary {
        let entries = list(s, &project_id(summary))?;
        if let Some(e) = entries.iter().find(|e| str_at(e, "/id") == reference)
            && let Some(draft) = e.get("draft")
        {
            return Ok(Loaded {
                draft: draft.clone(),
                id: Some(reference.to_owned()),
            });
        }
    }
    if std::path::Path::new(reference).is_file() {
        let text = std::fs::read_to_string(reference)
            .map_err(|e| CliError::usage(format!("{reference} cannot be read: {e}")))?;
        return Ok(Loaded {
            draft: draft_of_file(&text, reference)?,
            id: None,
        });
    }
    match summary {
        None => Err(CliError::usage(format!(
            "`{reference}` is not a file; to use a draft id give the project with --project DIR"
        ))),
        Some(summary) => {
            let ids: Vec<String> = list(s, &project_id(summary))?
                .iter()
                .map(|e| str_at(e, "/id").to_owned())
                .collect();
            Err(CliError::failed(format!(
                "no draft `{reference}` in this project (drafts: {})",
                if ids.is_empty() {
                    "none".to_owned()
                } else {
                    ids.join(", ")
                }
            )))
        }
    }
}

/// Saves a draft (a new one when `id` is `None`) and returns its id.
///
/// # Errors
/// The envelope of `designer_draft_save`.
pub(crate) fn save(
    s: &Session,
    project_id: &str,
    id: Option<&str>,
    draft: &Value,
) -> CliResult<String> {
    let mut request = json!({"projectId": project_id, "draft": draft});
    if let Some(id) = id {
        request["id"] = json!(id);
    }
    let reply = s.call("designer_draft_save", request)?;
    Ok(str_at(&reply.value, "/id").to_owned())
}

/// Runs a `drafts` command.
///
/// # Errors
/// The envelope of a failing command, a usage error for a bad `--set`.
pub(crate) fn run(s: &Session, cmd: &DraftsCmd) -> CliResult {
    match cmd {
        DraftsCmd::List { project: dir } => {
            let summary = project::open(s, dir)?;
            let entries = list(s, &project_id(&summary))?;
            s.emit(&json!({"drafts": entries}), || {
                if entries.is_empty() {
                    return "no drafts in this project\n".to_owned();
                }
                let mut t = Table::new(&["id", "kind", "defName", "label", "updatedAtMs"]);
                for e in &entries {
                    t.row(vec![
                        str_at(e, "/id").to_owned(),
                        str_at(e, "/kind").to_owned(),
                        str_at(e, "/defName").to_owned(),
                        str_at(e, "/label").to_owned(),
                        u64_at(e, "/updatedAtMs").to_string(),
                    ]);
                }
                t.render()
            });
        }
        DraftsCmd::Show {
            draft,
            project: dir,
        } => {
            let summary = project::open(s, dir)?;
            let loaded = load(s, draft, Some(&summary))?;
            s.print_json(&loaded.draft);
        }
        DraftsCmd::Set {
            draft,
            project: dir,
            set,
        } => {
            let summary = project::open(s, dir)?;
            let mut loaded = load(s, draft, Some(&summary))?;
            apply_sets(&mut loaded.draft, set).map_err(CliError::usage)?;
            let id = save(
                s,
                &project_id(&summary),
                loaded.id.as_deref(),
                &loaded.draft,
            )?;
            s.emit(&json!({"id": id, "draft": loaded.draft}), || {
                format!("saved draft {id}\n")
            });
        }
        DraftsCmd::Delete {
            draft,
            project: dir,
        } => {
            let summary = project::open(s, dir)?;
            let reply = s.call(
                "designer_draft_delete",
                json!({"projectId": project_id(&summary), "id": draft}),
            )?;
            s.emit(&reply.value, || {
                if bool_at(&reply.value, "/deleted") {
                    format!("deleted draft {draft}\n")
                } else {
                    format!("no draft {draft}\n")
                }
            });
        }
    }
    Ok(())
}
