#![allow(clippy::print_stdout, clippy::print_stderr)]
//! Headless RimStudio commands over the same registry as the desktop shell.
//!
//! The binary boots the application context of `rimstudio-app` and runs registry commands through its
//! dispatch, so it executes the same handlers as the shell (invariants I-01 and I-04). The generic
//! `call` command runs any registry row; the friendly commands wrap the rows of the 0.1.0 slice: the
//! settings and detection, the weapons designer and the Combat Extended patch generator.
//!
//! - [`cli`]: the command grammar.
//! - [`session`]: boot, running commands, progress and output.
//! - [`cmd`]: one module per command group.
//! - [`draftops`]: pure edits of draft JSON.
//! - [`fmt`] and [`error`]: text helpers and the error type with its exit codes.

mod cli;
mod cmd;
mod draftops;
mod error;
mod fmt;
mod session;

use std::io::Write as _;
use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Command};
use crate::error::{CliError, CliResult, EXIT_OK, EXIT_USAGE, EXIT_WARNINGS};
use crate::session::Session;

fn report(json: bool, error: &CliError) {
    let mut err = std::io::stderr().lock();
    if json {
        let text = serde_json::to_string(&error.to_json()).unwrap_or_default();
        let _ = writeln!(err, "{text}");
    } else {
        let _ = err.write_all(error.render().as_bytes());
    }
}

fn run(cli: &Cli) -> (CliResult, Option<Session>) {
    // These two need no application context, so they work even when boot would fail.
    match &cli.command {
        Command::Commands => return (cmd::generic::commands(cli.json), None),
        Command::Version => return (cmd::generic::version(cli.json), None),
        _ => {}
    }
    let session = match Session::open(cli.json, cli.no_progress) {
        Ok(s) => s,
        Err(e) => return (Err(e), None),
    };
    let result = cmd::run(&session, &cli.command);
    (result, Some(session))
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            let code = if e.use_stderr() { EXIT_USAGE } else { EXIT_OK };
            let _ = e.print();
            return ExitCode::from(code);
        }
    };
    let (result, session) = run(&cli);
    let code = match &result {
        Ok(()) => {
            if session.as_ref().is_some_and(Session::warned) {
                EXIT_WARNINGS
            } else {
                EXIT_OK
            }
        }
        Err(e) => {
            report(cli.json, e);
            e.exit_code()
        }
    };
    if let Some(session) = &session {
        session.close();
    }
    ExitCode::from(code)
}
