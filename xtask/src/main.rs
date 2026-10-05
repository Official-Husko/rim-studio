#![allow(clippy::print_stdout, clippy::print_stderr)]
#![allow(
    clippy::disallowed_methods,
    reason = "xtask is repository tooling: it spawns cargo and reads files"
)]
//! Repository automation for RimStudio (see docs/architecture/workspace-layout.md section 10).
//!
//! Subcommands: `check-layers`, `check-source`, `check-docs`, `check-all`.

mod docs;
mod jsonc;
mod layers;
mod model;
mod report;
mod source;

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Context, Result, bail};

use report::{Finding, print_findings};

const HELP: &str = "\
xtask: RimStudio repository checks

USAGE:
    cargo xtask [--root <dir>] <command>

COMMANDS:
    check-layers   layer tags, the dependency matrix and third party bans
    check-source   source scans: platform cfg, write fence, XML strings, printing, licence hygiene
    check-docs     documentation rules, plus dashes and emoji in Rust comments and manifests
    check-all      runs all of the above and prints a summary

OPTIONS:
    --root <dir>   repository root (default: found from the current directory)
    -h, --help     show this help

Exit status is 0 when every check passes and 1 otherwise; 2 means a usage or setup error.
";

/// A named check and the findings it produced.
struct Outcome {
    name: &'static str,
    findings: Vec<Finding>,
}

fn run_layers(root: &std::path::Path) -> Result<Outcome> {
    let text = std::fs::read_to_string(root.join("xtask/layers.jsonc"))
        .context("cannot read xtask/layers.jsonc")?;
    let cfg = layers::Config::parse(&text)?;
    let ws = model::Workspace::load(root)?;
    Ok(Outcome {
        name: "check-layers",
        findings: layers::check(&ws, &cfg),
    })
}

fn read_allow(root: &std::path::Path) -> Result<String> {
    std::fs::read_to_string(root.join("xtask/source-allow.jsonc"))
        .context("cannot read xtask/source-allow.jsonc")
}

fn run_source(root: &std::path::Path) -> Result<Outcome> {
    let allow = source::Allow::parse(&read_allow(root)?)?;
    Ok(Outcome {
        name: "check-source",
        findings: source::check(root, &allow),
    })
}

fn run_docs(root: &std::path::Path) -> Result<Outcome> {
    let allow = docs::DocsAllow::parse(&read_allow(root)?)?;
    Ok(Outcome {
        name: "check-docs",
        findings: docs::check(root, &allow),
    })
}

fn real_main(args: Vec<String>) -> Result<bool> {
    let mut root: Option<PathBuf> = None;
    let mut command: Option<String> = None;
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" | "help" => {
                print!("{HELP}");
                return Ok(true);
            }
            "--root" => root = Some(PathBuf::from(it.next().context("--root needs a value")?)),
            s if s.starts_with('-') => bail!("unknown option `{s}` (try --help)"),
            _ if command.is_none() => command = Some(a),
            _ => bail!("unexpected argument `{a}` (try --help)"),
        }
    }
    let Some(command) = command else {
        print!("{HELP}");
        return Ok(true);
    };
    let root = match root {
        Some(r) => r,
        None => {
            let cwd = std::env::current_dir().context("no current directory")?;
            model::find_root(&cwd).context("not inside the workspace; pass --root")?
        }
    };
    let mut outcomes = match command.as_str() {
        "check-layers" => vec![run_layers(&root)?],
        "check-source" => vec![run_source(&root)?],
        "check-docs" => vec![run_docs(&root)?],
        "check-all" => vec![run_layers(&root)?, run_source(&root)?, run_docs(&root)?],
        other => bail!("unknown command `{other}` (try --help)"),
    };
    let mut ok = true;
    for o in &mut outcomes {
        print_findings(&mut o.findings);
    }
    if command == "check-all" {
        println!("summary:");
        for o in &outcomes {
            let status = if o.findings.is_empty() {
                "ok"
            } else {
                "FAILED"
            };
            println!("  {:<13} {status} ({} findings)", o.name, o.findings.len());
        }
    }
    for o in &outcomes {
        ok &= o.findings.is_empty();
    }
    Ok(ok)
}

fn main() -> ExitCode {
    match real_main(std::env::args().skip(1).collect()) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(a: &[&str]) -> Vec<String> {
        a.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn help_succeeds() {
        assert!(real_main(args(&["--help"])).unwrap());
        assert!(real_main(args(&[])).unwrap());
    }

    #[test]
    fn unknown_command_and_option_are_usage_errors() {
        assert!(real_main(args(&["--root", ".", "frobnicate"])).is_err());
        assert!(real_main(args(&["--nope"])).is_err());
        assert!(real_main(args(&["--root"])).is_err());
        assert!(real_main(args(&["check-docs", "extra"])).is_err());
    }
}
