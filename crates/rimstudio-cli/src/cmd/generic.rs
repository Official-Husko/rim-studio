//! `call`, `commands` and `version`: the generic door to the registry.

use std::io::Read as _;

use rimstudio_app::dispatch_to_file;
use rimstudio_app::registry;
use serde_json::{Value, json};

use crate::cli::CallArgs;
use crate::error::{CliError, CliResult};
use crate::fmt::{Table, count_diagnostics, str_at};
use crate::session::{Session, absolute};

fn read_request(args: &CallArgs) -> CliResult<Value> {
    let given = [
        args.request.is_some(),
        args.data.is_some(),
        args.json_file.is_some(),
    ]
    .iter()
    .filter(|b| **b)
    .count();
    if given > 1 {
        return Err(CliError::usage(
            "give the request once: as an argument, with --data, or with --json-file",
        ));
    }
    let text = if let Some(file) = &args.json_file {
        if file == "-" {
            let mut text = String::new();
            std::io::stdin()
                .read_to_string(&mut text)
                .map_err(|e| CliError::usage(format!("standard input cannot be read: {e}")))?;
            text
        } else {
            std::fs::read_to_string(file)
                .map_err(|e| CliError::usage(format!("{file} cannot be read: {e}")))?
        }
    } else if let Some(text) = args.data.as_ref().or(args.request.as_ref()) {
        text.clone()
    } else {
        return Ok(json!({}));
    };
    serde_json::from_str(&text)
        .map_err(|e| CliError::usage(format!("the request is not valid JSON: {e}")))
}

pub(crate) fn request_problem(error: rimstudio_ipc_types::error::ApiError) -> CliError {
    match error.code.as_str() {
        "ipc.unknown-command" | "ipc.invalid-request" | "settings.invalid" => CliError::usage(
            format!("{} ({}): {}", error.code, error.error_id, error.message),
        ),
        _ => CliError::Api(error),
    }
}

/// `call COMMAND [REQUEST]`: runs one registry command and prints its JSON response.
///
/// # Errors
/// A usage error for an unknown command or a request that does not fit; the envelope otherwise.
pub(crate) fn call(s: &Session, args: &CallArgs) -> CliResult {
    let request = read_request(args)?;
    if let Some(out) = &args.out {
        let abs = absolute(out)?;
        let path = s.app.roots.data.join(&abs);
        let report =
            dispatch_to_file(&s.app, &args.command, request, &path).map_err(request_problem)?;
        let doc = json!({"path": report.path.as_str(), "bytes": report.bytes});
        s.emit(&doc, || {
            format!("wrote {} ({} bytes)\n", report.path, report.bytes)
        });
        return Ok(());
    }
    let reply = s.call(&args.command, request).map_err(request_problem)?;
    if count_diagnostics(&reply.diagnostics).warnings > 0 {
        s.note_warning();
    }
    s.print_json(&reply.value);
    Ok(())
}

/// `commands`: the registry table.
///
/// # Errors
/// Never fails.
pub(crate) fn commands(json: bool) -> CliResult {
    let table = registry::describe();
    if json {
        let text = serde_json::to_string_pretty(&table).unwrap_or_default();
        println!("{text}");
        return Ok(());
    }
    let mut t = Table::new(&["command", "kind", "request", "response"]);
    for row in table.as_array().map_or(&[][..], Vec::as_slice) {
        t.row(vec![
            str_at(row, "/name").to_owned(),
            str_at(row, "/kind").to_owned(),
            str_at(row, "/request").to_owned(),
            str_at(row, "/response").to_owned(),
        ]);
    }
    print!("{}", t.render());
    Ok(())
}

/// `version`: the version of the program and the hash of the command table.
///
/// # Errors
/// Never fails.
pub(crate) fn version(json: bool) -> CliResult {
    let doc = json!({
        "name": "rimstudio-cli",
        "version": env!("CARGO_PKG_VERSION"),
        "contractHash": registry::contract_hash(),
        "commandCount": registry::names().len(),
    });
    if json {
        println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_default());
    } else {
        println!(
            "rimstudio-cli {} (contract {}, {} commands)",
            env!("CARGO_PKG_VERSION"),
            str_at(&doc, "/contractHash"),
            registry::names().len()
        );
    }
    Ok(())
}
