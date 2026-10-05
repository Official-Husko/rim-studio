//! The command line grammar, declared with clap.
//!
//! Exit codes: 0 success, 1 error, 2 usage error, 3 completed with warnings. `--json` prints the
//! response of the underlying command as one JSON document; errors are then printed as
//! `{"error": {...}}` on standard error.

use clap::{Args, Parser, Subcommand, ValueEnum};

/// Help of the answers file of `convert plan` and `convert apply`.
const CONVERT_ANSWERS_HELP: &str = r#"ANSWERS FILE (--answers FILE)
  One JSON object. Members are the answers: ammoSet, defaultProjectile, weaponTagClass, oneHanded, beltFed,
  toolPenetration and overrides. In a file every number is an object with a value member, a tool
  penetration is a list of objects naming the tool by its label, and overrides is an object of such numbers:

    {
      "ammoSet": "RS_AmmoSetA",
      "weaponTagClass": "RS_CE_Class",
      "oneHanded": false,
      "beltFed": false,
      "toolPenetration": [ { "tool": "edge", "blunt": { "value": 2.0 } } ],
      "overrides": { "bulk": { "value": 6.5 }, "magazineSize": { "value": 30 } }
    }

  On the command line the same answers are --set keys: ammoSet=NAME, overrides.bulk=6.5 and
  toolPenetration.0.tool=edge together with toolPenetration.0.blunt=2.0 (the number is typed for you).

  For a mod of many weapons the file may also hold, besides the answers of one weapon:
    "default"  answers every weapon gets
    "groups"   a list of { "family": KEY or "defNames": [NAMES], "answers": { ... } }; a group applies to the
               weapons of that family (the family key is printed by `convert scan`) or to the named ones
    "defs"     { "DEFNAME": { ... } } answers of one weapon
  Later wins over earlier: default, then each group in order, then defs, then --set. Example:

    { "default": { "oneHanded": false, "beltFed": false },
      "groups": [ { "family": "ranged/RS_Rifle/RS_Shot00",
                    "answers": { "ammoSet": "RS_AmmoSetA", "weaponTagClass": "RS_CE_Class" } } ],
      "defs": { "RS_Special": { "overrides": { "bulk": { "value": 9.0 } } } } }"#;

/// Help of the Combat Extended keys of `designer plan` and `designer apply`.
const DESIGNER_CE_HELP: &str = r#"COMBAT EXTENDED VALUES (with --ce)
  Set them with --set on the draft: ce.ammoSet=NAME, ce.weaponTagClass=NAME, ce.bulk=6.5.
  A melee weapon needs a penetration for each tool, named by the tool label:
    --set ce.toolPenetration.0.tool=edge --set ce.toolPenetration.0.blunt=2.0
  N in ce.toolPenetration.N.tool is the position in the list (0 first); the next tool is N=1.
  In a draft JSON file the same entry is { "tool": "edge", "blunt": { "value": 2.0 } } and a number is an
  object with a value member."#;

/// The parsed command line.
#[derive(Debug, Parser)]
#[command(
    name = "rimstudio-cli",
    version,
    about = "Headless RimStudio: the weapons designer and the optional Combat Extended patch generator",
    long_about = "Headless RimStudio. Every command runs the same handlers as the desktop app.\n\nExit codes: 0 ok, 1 error, 2 usage, 3 completed with warnings.\n\nThe designer writes vanilla definitions only. A Combat Extended patch is generated only when you ask for it (--ce, or the convert commands), in its own file under a folder gated by LoadFolders.xml. Every command that writes needs an explicit --project folder and prints the files it will write; apply writes nothing without --yes.",
    propagate_version = true
)]
pub(crate) struct Cli {
    /// Print JSON documents instead of text.
    #[arg(long, global = true)]
    pub(crate) json: bool,
    /// Never draw the progress line on standard error.
    #[arg(long, global = true)]
    pub(crate) no_progress: bool,
    /// What to run.
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// The top level commands.
#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Run any registry command by name and print its JSON response.
    Call(CallArgs),
    /// List the commands of the registry.
    Commands,
    /// Print the version and the contract hash.
    Version,
    /// Show (or change) the detected game install, version, libraries and folders.
    Detect(DetectArgs),
    /// List, add and remove mod folders.
    Sources {
        /// The sources command.
        #[command(subcommand)]
        cmd: SourcesCmd,
    },
    /// Scan the mod library.
    Scan(ScanArgs),
    /// Read and change settings.
    Settings {
        /// The settings command.
        #[command(subcommand)]
        cmd: SettingsCmd,
    },
    /// Create or open a mod project folder.
    Project {
        /// The project command.
        #[command(subcommand)]
        cmd: ProjectCmd,
    },
    /// Search and resolve definitions.
    Defs {
        /// The defs command.
        #[command(subcommand)]
        cmd: DefsCmd,
    },
    /// The weapons designer (the same commands also work without the `designer` word).
    Designer {
        /// The designer command.
        #[command(subcommand)]
        cmd: DesignerCmd,
    },
    /// The designer commands at the top level.
    #[command(flatten)]
    Direct(DesignerCmd),
}

/// `call` arguments.
#[derive(Debug, Args)]
pub(crate) struct CallArgs {
    /// The command name, for example `settings_get`.
    pub(crate) command: String,
    /// The request as JSON text (`--json '{...}'` works too, it is the same positional).
    pub(crate) request: Option<String>,
    /// Read the request from a file (`-` is standard input).
    #[arg(long, value_name = "FILE")]
    pub(crate) json_file: Option<String>,
    /// The request as JSON text.
    #[arg(long, value_name = "JSON")]
    pub(crate) data: Option<String>,
    /// Write the response to this file (an absolute or relative path outside the app folders).
    #[arg(long, value_name = "FILE")]
    pub(crate) out: Option<String>,
}

/// `detect` arguments.
#[derive(Debug, Args)]
pub(crate) struct DetectArgs {
    /// Probe again instead of using the cached report.
    #[arg(long)]
    pub(crate) force: bool,
    /// Use this RimWorld folder as the game install.
    #[arg(long, value_name = "DIR")]
    pub(crate) install: Option<String>,
    /// Use this folder as the user data folder.
    #[arg(long, value_name = "DIR")]
    pub(crate) user_dir: Option<String>,
    /// Use this folder as the Steam root.
    #[arg(long, value_name = "DIR")]
    pub(crate) steam_root: Option<String>,
    /// Clear an override: `install`, `user-dir` or `steam-root`.
    #[arg(long, value_name = "FIELD")]
    pub(crate) clear: Vec<String>,
}

/// The `sources` commands.
#[derive(Debug, Subcommand)]
pub(crate) enum SourcesCmd {
    /// List the mod sources.
    List,
    /// Add a folder of mods.
    Add {
        /// The folder.
        path: String,
        /// A display name.
        #[arg(long)]
        label: Option<String>,
    },
    /// Remove a custom source by id.
    Remove {
        /// The source id (see `sources list`).
        id: String,
    },
}

/// `scan` arguments.
#[derive(Debug, Args)]
pub(crate) struct ScanArgs {
    /// Re-read everything and ignore the caches.
    #[arg(long)]
    pub(crate) full: bool,
}

/// The `settings` commands.
#[derive(Debug, Subcommand)]
pub(crate) enum SettingsCmd {
    /// Print the settings, or one value by dotted key.
    Get {
        /// A dotted key such as `appearance.density`.
        key: Option<String>,
    },
    /// Set one value by dotted key.
    Set {
        /// A dotted key such as `appearance.density`.
        key: String,
        /// The value (JSON, or plain text).
        value: String,
    },
    /// Return a value to its default.
    Reset {
        /// A dotted key.
        key: String,
    },
}

/// The `project` commands.
#[derive(Debug, Subcommand)]
pub(crate) enum ProjectCmd {
    /// Write the scaffold of a new mod (never overwrites a file).
    Create {
        /// The folder of the new mod.
        path: String,
        /// The mod name.
        #[arg(long)]
        name: String,
        /// The package id, for example `author.modname`.
        #[arg(long)]
        package_id: String,
        /// The author.
        #[arg(long, default_value = "")]
        author: String,
        /// A supported game version (repeatable). Default 1.6.
        #[arg(long = "game-version")]
        game_version: Vec<String>,
        /// One folder per game version plus `Common`, selected by `LoadFolders.xml`.
        #[arg(long)]
        versioned: bool,
        /// Also create the gated `Compat/CombatExtended/Patches` folder (never loads without Combat Extended).
        #[arg(long)]
        ce_folder: bool,
        /// Also create `Languages/English/Keyed`.
        #[arg(long)]
        languages: bool,
        /// Also create `Assemblies`.
        #[arg(long)]
        assemblies: bool,
        /// Also create `Source/Art` for art sources and code.
        #[arg(long)]
        source: bool,
        /// Also write a `.gitignore` for a mod repository.
        #[arg(long)]
        gitignore: bool,
        /// With `--gitignore`, keep `Source/Art` and `Raw Assets` out of the repository.
        #[arg(long)]
        ignore_source_art: bool,
        /// Also write a `README.md`.
        #[arg(long)]
        readme: bool,
        /// Also write a `Credits.txt`.
        #[arg(long)]
        credits: bool,
        /// Do not create the texture folders.
        #[arg(long)]
        no_textures: bool,
        /// Do not create `Sounds/Weapons`.
        #[arg(long)]
        no_sounds: bool,
    },
    /// Register an existing mod folder and print its summary.
    Open {
        /// The mod folder.
        path: String,
    },
    /// Print the annotated folder tree of a mod: roles, sizes, counts and layout issues.
    Tree {
        /// The mod folder.
        path: String,
        /// How many folder levels to print (the JSON output is always complete).
        #[arg(long, default_value_t = 3)]
        depth: usize,
        /// Also list files.
        #[arg(long)]
        files: bool,
    },
    /// Check a mod against the RimStudio mod layout and list the issues with a suggested fix each.
    Check {
        /// The mod folder.
        path: String,
    },
    /// Create the standard folders a mod lacks (folders only; never overwrites, moves or deletes).
    ScaffoldMissing {
        /// The mod folder.
        path: String,
        /// Only list what would be created.
        #[arg(long)]
        dry_run: bool,
    },
    /// Print one text file of a mod (size limited).
    Read {
        /// The mod folder.
        path: String,
        /// The file, relative to the mod folder.
        file: String,
        /// The most bytes to read.
        #[arg(long)]
        max_bytes: Option<u32>,
    },
}

/// The `defs` commands.
#[derive(Debug, Subcommand)]
pub(crate) enum DefsCmd {
    /// Search definitions by name, label or type.
    Search {
        /// The text to match.
        query: String,
        /// The session: `reference` (default), `ce`, or `project:<id>`.
        #[arg(long, default_value = "reference")]
        session: String,
        /// Search the session of this project folder instead.
        #[arg(long, value_name = "DIR")]
        project: Option<String>,
        /// Restrict to a def type (repeatable).
        #[arg(long = "type")]
        def_types: Vec<String>,
        /// Hide abstract definitions.
        #[arg(long)]
        hide_abstract: bool,
        /// Maximum rows.
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// First row.
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Print one definition after inheritance and patches.
    Resolve {
        /// The def type, for example `ThingDef`.
        def_type: String,
        /// The def name.
        def_name: String,
        /// The session: `reference` (default), `ce`, or `project:<id>`.
        #[arg(long, default_value = "reference")]
        session: String,
        /// Resolve in the session of this project folder instead.
        #[arg(long, value_name = "DIR")]
        project: Option<String>,
    },
}

/// The kind of weapon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum KindArg {
    /// A ranged weapon.
    Ranged,
    /// A melee weapon.
    Melee,
}

impl KindArg {
    /// The wire name.
    #[must_use]
    pub(crate) fn wire(self) -> &'static str {
        match self {
            Self::Ranged => "ranged",
            Self::Melee => "melee",
        }
    }
}

/// How strong a new design should be relative to its class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum StrengthArg {
    /// Toward the weak end of the class.
    Weaker,
    /// The class typical values.
    Typical,
    /// Toward the strong end of the class.
    Stronger,
}

/// A draft and the project it lives in.
#[derive(Debug, Args)]
pub(crate) struct DraftArgs {
    /// A draft id of the project (see `drafts list`) or the path of a draft JSON file.
    pub(crate) draft: String,
    /// The mod project folder.
    #[arg(long, value_name = "DIR")]
    pub(crate) project: Option<String>,
    /// Override a value for this run only: `key=value` (repeatable), for example `ranged.damage=12`.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    pub(crate) set: Vec<String>,
}

/// `designer projectile` arguments.
#[derive(Debug, Args)]
pub(crate) struct ProjectileArgs {
    /// The draft and project.
    #[command(flatten)]
    pub(crate) draft: DraftArgs,
    /// Point the weapon back at the shared projectile it was copied from instead of giving it its own.
    #[arg(long)]
    pub(crate) shared: bool,
}

/// `designer new` arguments.
#[derive(Debug, Args)]
pub(crate) struct NewArgs {
    /// Ranged or melee.
    pub(crate) kind: KindArg,
    /// The definition name of the new weapon.
    #[arg(long)]
    pub(crate) name: String,
    /// The mod project folder the draft belongs to.
    #[arg(long, value_name = "DIR")]
    pub(crate) project: String,
    /// The display label (default derived from the name).
    #[arg(long)]
    pub(crate) label: Option<String>,
    /// Fill the empty numbers from the class pool, and the parent, projectile and cost list from the most
    /// similar reference weapon (as suggestions).
    #[arg(long, value_enum, conflicts_with = "from")]
    pub(crate) strength: Option<StrengthArg>,
    /// Clone this loaded weapon: every field of it is copied, the source becomes the first anchor and the
    /// new weapon is named by `--name`. The clone is vanilla; Combat Extended stays off.
    #[arg(long, value_name = "DEFNAME")]
    pub(crate) from: Option<String>,
    /// The prefix the mod uses for definition names (added to `--name` when it lacks it).
    #[arg(long, value_name = "PREFIX", requires = "from")]
    pub(crate) prefix: Option<String>,
    /// Keep pointing at the projectile of the source instead of giving the clone a projectile of its own.
    /// With the shared projectile, a damage change is shown in the readouts but not written to the files.
    #[arg(long, requires = "from")]
    pub(crate) shared_projectile: bool,
    /// Set a value: `key=value` (repeatable), for example `ranged.damage=12`.
    #[arg(long = "set", value_name = "KEY=VALUE")]
    pub(crate) set: Vec<String>,
}

/// `designer plan` arguments.
#[derive(Debug, Args)]
pub(crate) struct PlanArgs {
    /// The draft and project.
    #[command(flatten)]
    pub(crate) draft: DraftArgs,
    /// Also plan the optional Combat Extended patch and the LoadFolders.xml edit.
    #[arg(long)]
    pub(crate) ce: bool,
    /// Fill the empty fields of the Combat Extended block from the suggestions (`designer ce-suggest`):
    /// every field rated reliable or rough, or only the named FIELDs (for example `bulk reloadTime`).
    /// Typed values are never overwritten, and the ammo set and weapon tag class are never chosen for you.
    /// Put it after the draft: it takes the names that follow it.
    #[arg(long, value_name = "FIELD", num_args = 0.., requires = "ce")]
    pub(crate) accept_suggestions: Option<Vec<String>>,
}

/// `designer apply` arguments.
#[derive(Debug, Args)]
pub(crate) struct ApplyArgs {
    /// The draft and project.
    #[command(flatten)]
    pub(crate) draft: DraftArgs,
    /// Also write the optional Combat Extended patch and the LoadFolders.xml edit.
    #[arg(long)]
    pub(crate) ce: bool,
    /// Fill the empty fields of the Combat Extended block from the suggestions, as for `plan`.
    #[arg(long, value_name = "FIELD", num_args = 0.., requires = "ce")]
    pub(crate) accept_suggestions: Option<Vec<String>>,
    /// Write the files. Without it nothing is written.
    #[arg(long)]
    pub(crate) yes: bool,
    /// Do not back up files that are replaced.
    #[arg(long)]
    pub(crate) no_backup: bool,
    /// Skip the dry run of the generated patch.
    #[arg(long)]
    pub(crate) no_dry_apply: bool,
}

/// `designer quiz` arguments.
#[derive(Debug, Args)]
pub(crate) struct QuizArgs {
    /// A draft id of the project or the path of a draft JSON file (imported into the project).
    pub(crate) draft: String,
    /// The mod project folder.
    #[arg(long, value_name = "DIR")]
    pub(crate) project: String,
    /// Answers from a file: a JSON array of answers in order, or an object by question id.
    #[arg(long, value_name = "FILE")]
    pub(crate) answers: Option<String>,
}

/// The `drafts` commands.
#[derive(Debug, Subcommand)]
pub(crate) enum DraftsCmd {
    /// List the drafts of a project.
    List {
        /// The mod project folder.
        #[arg(long, value_name = "DIR")]
        project: String,
    },
    /// Print one draft.
    Show {
        /// The draft id.
        draft: String,
        /// The mod project folder.
        #[arg(long, value_name = "DIR")]
        project: String,
    },
    /// Change values of a stored draft.
    Set {
        /// The draft id.
        draft: String,
        /// The mod project folder.
        #[arg(long, value_name = "DIR")]
        project: String,
        /// `key=value` (repeatable).
        #[arg(long = "set", value_name = "KEY=VALUE", required = true)]
        set: Vec<String>,
    },
    /// Delete a draft.
    Delete {
        /// The draft id.
        draft: String,
        /// The mod project folder.
        #[arg(long, value_name = "DIR")]
        project: String,
    },
}

/// `convert plan` and `convert apply` selection arguments.
#[derive(Debug, Args)]
pub(crate) struct ConvertTarget {
    /// The mod project folder whose weapons are converted.
    #[arg(long, value_name = "DIR")]
    pub(crate) project: String,
    /// A weapon definition to convert (repeatable).
    #[arg(long = "def", value_name = "DEFNAME")]
    pub(crate) defs: Vec<String>,
    /// Convert every weapon that can be converted.
    #[arg(long)]
    pub(crate) all: bool,
    /// Answers from a JSON file: an answers object, or an object with `default`, `groups` (one answer set
    /// for a weapon family or a list of names) and `defs` (one set per weapon). See the examples below.
    #[arg(long, value_name = "FILE")]
    pub(crate) answers: Option<String>,
    /// One answer: `ammoSet=NAME`, `defaultProjectile=NAME`, `weaponTagClass=NAME`, `oneHanded=true`,
    /// `beltFed=true` or `overrides.<field>=value` (repeatable).
    #[arg(long = "set", value_name = "KEY=VALUE")]
    pub(crate) set: Vec<String>,
}

/// The `convert` commands (the automatic Combat Extended patch generator for an existing mod).
#[derive(Debug, Subcommand)]
pub(crate) enum ConvertCmd {
    /// List the weapons of a mod with their conversion status and open questions.
    Scan {
        /// The mod project folder.
        #[arg(long, value_name = "DIR")]
        project: String,
        /// Hide weapons that already have a conversion.
        #[arg(long)]
        hide_converted: bool,
    },
    /// Show the patch files a conversion would write; writes nothing.
    #[command(after_long_help = CONVERT_ANSWERS_HELP)]
    Plan(ConvertTarget),
    /// Write the conversion. Without `--yes` it only prints the plan.
    #[command(after_long_help = CONVERT_ANSWERS_HELP)]
    Apply {
        /// What to convert.
        #[command(flatten)]
        target: ConvertTarget,
        /// Write the files. Without it nothing is written.
        #[arg(long)]
        yes: bool,
        /// Do not back up files that are replaced.
        #[arg(long)]
        no_backup: bool,
        /// Skip the dry run of the generated patch.
        #[arg(long)]
        no_dry_apply: bool,
    },
}

/// The `lint` commands.
#[derive(Debug, Subcommand)]
pub(crate) enum LintCmd {
    /// Check the Combat Extended conversions of one or more mod folders.
    Ce {
        /// Mod folders.
        #[arg(required = true)]
        paths: Vec<String>,
    },
}

/// The designer commands.
#[derive(Debug, Subcommand)]
pub(crate) enum DesignerCmd {
    /// List the reference weapons of a kind.
    Refs {
        /// Ranged or melee.
        kind: KindArg,
        /// Restrict to a role.
        #[arg(long)]
        role: Option<String>,
        /// Restrict to a tech level (neolithic, medieval, industrial, spacer, ultra, archotech).
        #[arg(long)]
        tier: Option<String>,
        /// Maximum rows.
        #[arg(long, default_value_t = 50)]
        limit: u32,
        /// First row.
        #[arg(long, default_value_t = 0)]
        offset: u32,
    },
    /// Build the pools and measure the estimates (cached).
    Calibrate {
        /// Ranged or melee.
        #[arg(default_value = "ranged")]
        kind: KindArg,
        /// Ignore the cache.
        #[arg(long)]
        force: bool,
        /// Worker threads (default automatic).
        #[arg(long)]
        threads: Option<u8>,
    },
    /// Create a draft in a project's draft store.
    New(NewArgs),
    /// Show the exact readouts, suggestions and diagnostics of a draft.
    Preview(DraftArgs),
    /// Show what a cloned draft changes against its source weapon, and the effect on the readouts.
    Diff(DraftArgs),
    /// Give a gun draft a projectile of its own (copied from the one it fires), or point it back at the
    /// shared one with `--shared`; the draft is saved.
    Projectile(ProjectileArgs),
    /// Suggest the Combat Extended numbers of a draft from your own conversions; changes nothing.
    CeSuggest(DraftArgs),
    /// Show the files a draft would write, with diffs; writes nothing.
    #[command(after_long_help = DESIGNER_CE_HELP)]
    Plan(PlanArgs),
    /// Write the files of a draft. Without `--yes` it only prints the plan.
    #[command(after_long_help = DESIGNER_CE_HELP)]
    Apply(ApplyArgs),
    /// Answer the estimate quiz for a draft (interactive on a terminal, or from a file).
    Quiz(QuizArgs),
    /// Manage the drafts of a project.
    Drafts {
        /// The drafts command.
        #[command(subcommand)]
        cmd: DraftsCmd,
    },
    /// Convert an existing mod's weapons to Combat Extended.
    Convert {
        /// The convert command.
        #[command(subcommand)]
        cmd: ConvertCmd,
    },
    /// Lint Combat Extended patches.
    Lint {
        /// The lint command.
        #[command(subcommand)]
        cmd: LintCmd,
    },
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::*;

    #[test]
    fn the_grammar_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn designer_commands_work_with_and_without_the_group_word() {
        let a = Cli::try_parse_from(["rimstudio-cli", "designer", "refs", "ranged"]);
        let b = Cli::try_parse_from(["rimstudio-cli", "refs", "ranged"]);
        assert!(a.is_ok(), "{a:?}");
        assert!(b.is_ok(), "{b:?}");
    }

    #[test]
    fn json_may_come_before_or_after_the_subcommand() {
        let a = Cli::try_parse_from(["rimstudio-cli", "--json", "scan"]).map(|c| c.json);
        let b = Cli::try_parse_from(["rimstudio-cli", "scan", "--json"]).map(|c| c.json);
        assert!(a.unwrap_or(false));
        assert!(b.unwrap_or(false));
    }

    #[test]
    fn call_accepts_the_request_after_the_json_flag() {
        let c = Cli::try_parse_from([
            "rimstudio-cli",
            "call",
            "app_ping",
            "--json",
            "{\"echo\":\"x\"}",
        ]);
        match c {
            Ok(Cli {
                json: true,
                command: Command::Call(a),
                ..
            }) => {
                assert_eq!(a.request.as_deref(), Some("{\"echo\":\"x\"}"));
            }
            other => panic!("{other:?}"),
        }
    }
}
