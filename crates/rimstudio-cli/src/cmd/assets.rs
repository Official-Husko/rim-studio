//! `designer asset`: the texture and sound imports of a draft, and the facts of one file.
//!
//! `designer asset DRAFT --project DIR --texture PNG --shot-clip WAV ...` stores the source paths and the
//! sound settings in the draft (nothing is copied until `designer apply`), then prints what the designer
//! knows about each file. `designer asset info PATH` prints the facts of one file without a draft. Paths
//! given on the command line are made absolute against the working directory before they are stored.

use std::fmt::Write as _;

use clap::{Args, Subcommand};
use serde_json::{Value, json};

use super::{drafts, project};
use crate::error::{CliError, CliResult};
use crate::fmt::{arr_at, bytes, count_diagnostics, render_diagnostics, str_at, u64_at};
use crate::session::{Session, absolute};

/// `designer asset` arguments: either the edit of a draft or the `info` subcommand.
#[derive(Debug, Args)]
#[command(args_conflicts_with_subcommands = true)]
pub(crate) struct AssetArgs {
    /// Show the facts of one file instead of editing a draft.
    #[command(subcommand)]
    pub(crate) cmd: Option<AssetCmd>,
    /// A draft id of the project (see `drafts list`).
    pub(crate) draft: Option<String>,
    /// The mod project folder.
    #[arg(long, value_name = "DIR")]
    pub(crate) project: Option<String>,
    /// The PNG for the weapon. It is copied to `Textures/Things/Item/Equipment/...` on apply.
    #[arg(long, value_name = "PNG")]
    pub(crate) texture: Option<String>,
    /// The PNG for the weapon's own projectile (the draft needs a projectile of its own).
    #[arg(long, value_name = "PNG")]
    pub(crate) projectile_texture: Option<String>,
    /// A clip for the custom shot sound (WAV or OGG). Repeatable; the game plays one of them at random.
    #[arg(long = "shot-clip", value_name = "FILE")]
    pub(crate) shot_clip: Vec<String>,
    /// The def name of the custom shot sound (default `<DefName>_Shot`).
    #[arg(long, value_name = "NAME")]
    pub(crate) shot_name: Option<String>,
    /// The volume range of the custom shot sound, `MIN~MAX` (50 is the game's default).
    #[arg(long, value_name = "MIN~MAX")]
    pub(crate) shot_volume: Option<String>,
    /// The pitch range of the custom shot sound, `MIN~MAX` (1 is the clip's own pitch).
    #[arg(long, value_name = "MIN~MAX")]
    pub(crate) shot_pitch: Option<String>,
    /// The distance range in tiles in which the shot is heard, `MIN~MAX`.
    #[arg(long, value_name = "MIN~MAX")]
    pub(crate) shot_distance: Option<String>,
    /// How many instances of the shot sound may play at once.
    #[arg(long, value_name = "N")]
    pub(crate) shot_max: Option<u32>,
    /// Remove the weapon texture import.
    #[arg(long)]
    pub(crate) clear_texture: bool,
    /// Remove the projectile texture import.
    #[arg(long)]
    pub(crate) clear_projectile_texture: bool,
    /// Remove the custom shot sound.
    #[arg(long)]
    pub(crate) clear_shot_sound: bool,
}

/// The subcommands of `designer asset`.
#[derive(Debug, Subcommand)]
pub(crate) enum AssetCmd {
    /// Show the facts of one texture or sound clip: format, size, hash, dimensions, problems.
    Info(AssetInfoArgs),
}

/// `designer asset info` arguments.
#[derive(Debug, Args)]
pub(crate) struct AssetInfoArgs {
    /// The file.
    pub(crate) path: String,
    /// A mod project folder, for a path relative to the project.
    #[arg(long, value_name = "DIR")]
    pub(crate) project: Option<String>,
}

fn range(text: &str, flag: &str) -> CliResult<Value> {
    let bad = || {
        CliError::usage(format!(
            "{flag} takes MIN~MAX, for example 30~34 (got {text:?})"
        ))
    };
    let (a, b) = text.split_once('~').unwrap_or((text, text));
    let min: f64 = a.trim().parse().map_err(|_| bad())?;
    let max: f64 = b.trim().parse().map_err(|_| bad())?;
    Ok(json!({"min": min, "max": max}))
}

fn render_info(v: &Value) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}", str_at(v, "/path"));
    match str_at(v, "/status") {
        "found" => {
            let mut line = format!("  {} {}", str_at(v, "/kind"), bytes(u64_at(v, "/bytes")));
            if v.get("width").is_some() {
                let _ = write!(
                    line,
                    ", {} x {} pixels",
                    u64_at(v, "/width"),
                    u64_at(v, "/height")
                );
            }
            if v.get("channels").is_some() {
                let _ = write!(line, ", {} channel(s)", u64_at(v, "/channels"));
            }
            if v.get("sampleRate").is_some() {
                let _ = write!(line, ", {} Hz", u64_at(v, "/sampleRate"));
            }
            if v.get("durationMs").is_some() {
                let _ = write!(line, ", {} ms", u64_at(v, "/durationMs"));
            }
            let _ = writeln!(out, "{line}");
            let _ = writeln!(out, "  sha256 {}", str_at(v, "/sha256"));
        }
        "missing" => out.push_str("  the file does not exist\n"),
        "refused" => out.push_str("  not accepted: a link, a folder or an unreadable file\n"),
        _ => {
            let _ = writeln!(out, "  too large: {}", bytes(u64_at(v, "/bytes")));
        }
    }
    out.push_str(&render_diagnostics(arr_at(v, "/diagnostics")));
    out
}

/// `designer asset info PATH [--project DIR]`: the facts of one file; changes nothing.
///
/// # Errors
/// The envelope of `designer_asset_info`.
pub(crate) fn info(s: &Session, args: &AssetInfoArgs) -> CliResult {
    let mut request = json!({"path": absolute_unless_project(&args.path, args.project.is_some())?});
    if let Some(dir) = &args.project {
        let summary = project::open(s, dir)?;
        request["projectId"] = json!(drafts::project_id(&summary));
    }
    let reply = s.call("designer_asset_info", request)?;
    let c = count_diagnostics(arr_at(&reply.value, "/diagnostics"));
    s.emit(&reply.value, || render_info(&reply.value));
    if c.errors + c.warnings > 0 || str_at(&reply.value, "/status") != "found" {
        s.note_warning();
    }
    Ok(())
}

/// With a project a relative path is the project's; without one it is made absolute here.
fn absolute_unless_project(path: &str, has_project: bool) -> CliResult<String> {
    if has_project && !std::path::Path::new(path).is_absolute() {
        Ok(path.to_owned())
    } else {
        absolute(path)
    }
}

fn set_object<'a>(spec: &'a mut Value, key: &str) -> &'a mut Value {
    if !spec.get(key).is_some_and(Value::is_object) {
        spec[key] = json!({});
    }
    &mut spec[key]
}

/// Applies the flags to the spec of a draft. Returns true when anything changed.
fn edit_spec(spec: &mut Value, args: &AssetArgs) -> CliResult<bool> {
    let mut changed = false;
    if args.clear_texture {
        if let Some(a) = spec.get_mut("assets").and_then(Value::as_object_mut) {
            a.remove("texture");
        }
        changed = true;
    }
    if args.clear_projectile_texture {
        if let Some(a) = spec.get_mut("assets").and_then(Value::as_object_mut) {
            a.remove("projectileTexture");
        }
        changed = true;
    }
    if let Some(path) = &args.texture {
        set_object(spec, "assets")["texture"] = json!(absolute(path)?);
        changed = true;
    }
    if let Some(path) = &args.projectile_texture {
        set_object(spec, "assets")["projectileTexture"] = json!(absolute(path)?);
        changed = true;
    }
    if args.clear_shot_sound {
        if let Some(a) = spec.get_mut("sounds").and_then(Value::as_object_mut) {
            a.remove("shot");
        }
        changed = true;
    }
    let sound_flags = !args.shot_clip.is_empty()
        || args.shot_name.is_some()
        || args.shot_volume.is_some()
        || args.shot_pitch.is_some()
        || args.shot_distance.is_some()
        || args.shot_max.is_some();
    if sound_flags {
        let sounds = set_object(spec, "sounds");
        let shot = set_object(sounds, "shot");
        if !args.shot_clip.is_empty() {
            let clips = args
                .shot_clip
                .iter()
                .map(|c| absolute(c).map(Value::from))
                .collect::<CliResult<Vec<Value>>>()?;
            shot["clips"] = Value::Array(clips);
        }
        if let Some(name) = &args.shot_name {
            shot["defName"] = json!(name);
        }
        if let Some(v) = &args.shot_volume {
            shot["volume"] = range(v, "--shot-volume")?;
        }
        if let Some(v) = &args.shot_pitch {
            shot["pitch"] = range(v, "--shot-pitch")?;
        }
        if let Some(v) = &args.shot_distance {
            shot["distance"] = range(v, "--shot-distance")?;
        }
        if let Some(n) = args.shot_max {
            shot["maxSimultaneous"] = json!(n);
        }
        changed = true;
    }
    Ok(changed)
}

/// The source paths a spec imports, with a role label.
fn imports_of(spec: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let Some(p) = spec.pointer("/assets/texture").and_then(Value::as_str) {
        out.push(("weapon texture".to_owned(), p.to_owned()));
    }
    if let Some(p) = spec
        .pointer("/assets/projectileTexture")
        .and_then(Value::as_str)
    {
        out.push(("projectile texture".to_owned(), p.to_owned()));
    }
    for clip in arr_at(spec, "/sounds/shot/clips") {
        if let Some(p) = clip.as_str() {
            out.push(("shot clip".to_owned(), p.to_owned()));
        }
    }
    out
}

/// `designer asset DRAFT --project DIR [flags]`: stores the imports in the draft (saved), then shows the
/// facts of every imported file. Nothing is copied before `designer apply`.
///
/// # Errors
/// A usage error for a missing draft or project or a bad range, or the envelope of a failing command.
pub(crate) fn edit(s: &Session, args: &AssetArgs) -> CliResult {
    let Some(reference) = &args.draft else {
        return Err(CliError::usage(
            "name a draft (designer asset DRAFT --project DIR ...) or use `designer asset info PATH`",
        ));
    };
    let Some(dir) = &args.project else {
        return Err(CliError::usage(
            "this command needs the mod project folder: add --project DIR",
        ));
    };
    let summary = project::open(s, dir)?;
    let pid = drafts::project_id(&summary);
    let mut loaded = drafts::load(s, reference, Some(&summary))?;
    let changed = {
        let spec = loaded
            .draft
            .get_mut("spec")
            .ok_or_else(|| CliError::usage("the draft has no spec"))?;
        edit_spec(spec, args)?
    };
    let id = if changed {
        drafts::save(s, &pid, loaded.id.as_deref(), &loaded.draft)?
    } else {
        loaded.id.clone().unwrap_or_default()
    };
    let spec = loaded.draft.get("spec").cloned().unwrap_or(Value::Null);
    let mut facts = Vec::new();
    for (role, path) in imports_of(&spec) {
        let reply = s.call(
            "designer_asset_info",
            json!({"path": path, "projectId": pid}),
        )?;
        facts.push((role, reply.value));
    }
    let doc = json!({
        "id": id,
        "saved": changed,
        "assets": spec.get("assets").cloned().unwrap_or(Value::Null),
        "sounds": spec.get("sounds").cloned().unwrap_or(Value::Null),
        "files": facts.iter().map(|(r, v)| json!({"role": r, "info": v})).collect::<Vec<_>>(),
    });
    let mut problems = 0;
    for (_, v) in &facts {
        let c = count_diagnostics(arr_at(v, "/diagnostics"));
        problems += c.errors + c.warnings;
        if str_at(v, "/status") != "found" {
            problems += 1;
        }
    }
    s.emit(&doc, || {
        let mut out = String::new();
        if changed {
            let _ = writeln!(out, "saved draft {id}");
        }
        if facts.is_empty() {
            out.push_str("no texture or sound is imported by this draft\n");
        }
        for (role, v) in &facts {
            let _ = writeln!(out, "{role}:");
            out.push_str(&render_info(v));
        }
        if !facts.is_empty() {
            out.push_str("nothing is copied until `designer apply` writes the plan\n");
        }
        out
    });
    if problems > 0 {
        s.note_warning();
    }
    Ok(())
}

/// Runs `designer asset`.
///
/// # Errors
/// See [`info`] and [`edit`].
pub(crate) fn run(s: &Session, args: &AssetArgs) -> CliResult {
    match &args.cmd {
        Some(AssetCmd::Info(a)) => info(s, a),
        None => edit(s, args),
    }
}
