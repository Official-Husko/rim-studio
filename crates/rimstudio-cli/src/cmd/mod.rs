//! The commands, one module per group, and the table that routes a parsed command to its module.

pub(crate) mod ammo;
pub(crate) mod archetype;
pub(crate) mod assets;
pub(crate) mod convert;
pub(crate) mod defs;
pub(crate) mod designer;
pub(crate) mod detect;
pub(crate) mod drafts;
pub(crate) mod generic;
pub(crate) mod lint;
pub(crate) mod project;
pub(crate) mod project_about;
pub(crate) mod project_fix;
pub(crate) mod project_layout;
pub(crate) mod project_link;
pub(crate) mod scan;
pub(crate) mod settings;
pub(crate) mod sources;

use crate::cli::{Command, DesignerCmd};
use crate::error::CliResult;
use crate::session::Session;

/// Runs a parsed command against a booted session.
///
/// # Errors
/// Whatever the command reports.
pub(crate) fn run(s: &Session, command: &Command) -> CliResult {
    match command {
        Command::Call(args) => generic::call(s, args),
        // Handled before boot; reaching them here still works.
        Command::Commands => generic::commands(s.json),
        Command::Version => generic::version(s.json),
        Command::Detect(args) => detect::run(s, args),
        Command::Sources { cmd } => sources::run(s, cmd),
        Command::Scan(args) => scan::run(s, args),
        Command::Settings { cmd } => settings::run(s, cmd),
        Command::Project { cmd } => project::run(s, cmd),
        Command::Library { cmd } => match cmd {
            crate::cli::LibraryCmd::Search { query, limit } => {
                project_about::library_search(s, query, *limit)
            }
        },
        Command::Defs { cmd } => defs::run(s, cmd),
        Command::Designer { cmd } | Command::Direct(cmd) => run_designer(s, cmd),
    }
}

/// Runs a designer command.
///
/// # Errors
/// Whatever the command reports.
pub(crate) fn run_designer(s: &Session, command: &DesignerCmd) -> CliResult {
    match command {
        DesignerCmd::Refs {
            kind,
            role,
            tier,
            limit,
            offset,
        } => designer::refs(s, *kind, role.as_deref(), tier.as_deref(), *limit, *offset),
        DesignerCmd::Calibrate {
            kind,
            force,
            threads,
        } => designer::calibrate(s, *kind, *force, *threads),
        DesignerCmd::New(args) => designer::new_draft_cmd(s, args),
        DesignerCmd::Preview(args) => designer::preview(s, args),
        DesignerCmd::Diff(args) => designer::diff(s, args),
        DesignerCmd::Projectile(args) => designer::projectile(s, args),
        DesignerCmd::Asset(args) => assets::run(s, args),
        DesignerCmd::Ammo(args) => ammo::run(s, args),
        DesignerCmd::Archetypes(args) => archetype::list(s, args),
        DesignerCmd::Propose(args) => archetype::propose(s, args),
        DesignerCmd::CeSuggest(args) => designer::ce_suggest(s, args),
        DesignerCmd::Plan(args) => designer::plan(s, args),
        DesignerCmd::Apply(args) => designer::apply(s, args),
        DesignerCmd::Quiz(args) => designer::quiz(s, args),
        DesignerCmd::Drafts { cmd } => drafts::run(s, cmd),
        DesignerCmd::Convert { cmd } => convert::run(s, cmd),
        DesignerCmd::Lint { cmd } => lint::run(s, cmd),
    }
}
