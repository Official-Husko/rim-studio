//! `lint ce PATH...`: checks the Combat Extended patch files of mod folders, or single patch files.
//!
//! The command runs `designer_lint_files`: every patch file of a mod folder (any XML file below a `Patches`
//! folder and the patch files of the gated Combat Extended folder, hand written or generated), or the one
//! file named. A file is looked up in the mod folder that holds it (the nearest parent with an
//! `About/About.xml`). The command reports every finding and writes nothing. The rules that need Combat
//! Extended data are listed as not checked when no install with Combat Extended is selected.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::cli::LintCmd;
use crate::cmd::drafts::project_id;
use crate::cmd::project;
use crate::error::{CliError, CliResult};
use crate::fmt::{arr_at, bool_at, str_at, u64_at};
use crate::session::{Session, absolute};

/// The mod folder that holds a file, and the file relative to it with `/` separators.
fn mod_root_of(file: &Path) -> Option<(PathBuf, String)> {
    let mut dir = file.parent()?;
    loop {
        if dir.join("About").join("About.xml").is_file() {
            let rel = file.strip_prefix(dir).ok()?;
            let rel: Vec<String> = rel
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect();
            return Some((dir.to_path_buf(), rel.join("/")));
        }
        dir = dir.parent()?;
    }
}

/// Splits an argument into the folder to open and the files to check (none means every patch file).
fn target_of(arg: &str) -> CliResult<(String, Vec<String>)> {
    let abs = absolute(arg)?;
    let path = Path::new(&abs);
    if path.is_file() {
        let (root, rel) = mod_root_of(path).ok_or_else(|| {
            CliError::usage(format!(
                "{arg} is not inside a mod folder (a folder with About/About.xml)"
            ))
        })?;
        return Ok((root.to_string_lossy().into_owned(), vec![rel]));
    }
    Ok((abs, Vec::new()))
}

fn render_finding(f: &Value) -> String {
    let mut place = String::new();
    if let Some(op) = f.get("operation").and_then(Value::as_u64) {
        let _ = write!(place, " operation {op}");
    }
    if let Some(x) = f.get("xpath").and_then(Value::as_str) {
        let _ = write!(place, " xpath {x}");
    }
    let rule = f
        .get("ruleId")
        .and_then(Value::as_str)
        .unwrap_or_else(|| str_at(f, "/code"));
    format!(
        "  {} {rule}{place}: {}\n",
        str_at(f, "/severity"),
        str_at(f, "/message")
    )
}

fn render_result(root: &str, lint: &Value, explain: bool) -> String {
    let counts = &lint["counts"];
    let mut text = format!(
        "{root}: {} errors, {} warnings, {} notes, {} files\n",
        u64_at(counts, "/errors"),
        u64_at(counts, "/warnings"),
        u64_at(counts, "/notes"),
        u64_at(counts, "/files"),
    );
    for f in arr_at(lint, "/project") {
        text.push_str(&render_finding(f));
    }
    for file in arr_at(lint, "/files") {
        let status = str_at(file, "/status");
        let _ = writeln!(
            text,
            "{} ({status}, {} operations)",
            str_at(file, "/path"),
            u64_at(file, "/operations")
        );
        for f in arr_at(file, "/findings") {
            text.push_str(&render_finding(f));
            if explain && let Some(e) = f.get("explanation").and_then(Value::as_str) {
                let _ = writeln!(text, "      {e}");
            }
        }
    }
    for n in arr_at(lint, "/notChecked") {
        let _ = writeln!(
            text,
            "  not checked {}: {}",
            str_at(n, "/ruleId"),
            str_at(n, "/reason")
        );
    }
    if !bool_at(lint, "/ceData") {
        text.push_str(
            "  no Combat Extended data is loaded, so the data dependent rules did not run\n",
        );
    }
    text
}

/// Runs a `lint` command.
///
/// # Errors
/// The envelope of a failing command; a reported failure (exit 1) when an error finding was found.
pub(crate) fn run(s: &Session, cmd: &LintCmd) -> CliResult {
    let LintCmd::Ce { paths, explain } = cmd;
    let mut results: Vec<Value> = Vec::new();
    let mut text = String::new();
    let (mut errors, mut warnings) = (0u64, 0u64);
    for arg in paths {
        let (root, files) = target_of(arg)?;
        let summary = project::open(s, &root)?;
        let lint = s
            .call(
                "designer_lint_files",
                json!({"projectId": project_id(&summary), "paths": files}),
            )?
            .value;
        errors += u64_at(&lint["counts"], "/errors");
        warnings += u64_at(&lint["counts"], "/warnings");
        text.push_str(&render_result(str_at(&summary, "/path"), &lint, *explain));
        let mut entry = lint;
        if let Some(obj) = entry.as_object_mut() {
            obj.insert("path".to_owned(), json!(str_at(&summary, "/path")));
        }
        results.push(entry);
    }
    s.emit(
        &json!({"results": results, "errors": errors, "warnings": warnings}),
        || text.clone(),
    );
    if errors > 0 {
        return Err(CliError::Reported);
    }
    if warnings > 0 {
        s.note_warning();
    }
    Ok(())
}
