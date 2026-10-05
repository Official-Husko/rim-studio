//! The layout check: what in a project differs from the RimStudio mod layout v1, as a list of issues with a
//! suggested fix.
//!
//! The check is read only. Only one kind of fix is automatic, creating a missing standard folder, and it is
//! always safe: nothing is moved, replaced or deleted. Every other fix is a suggestion the user carries out
//! (the tool never reorganises existing files without an explicit action; layout spec section 8).
//!
//! The codes are stable: `layout.missing-about`, `layout.missing-folder`, `layout.folder-case`,
//! `layout.weapon-misplaced`, `layout.def-wrong-category`, `layout.ce-outside-gate`,
//! `layout.ce-folder-ungated`, `layout.ce-legacy-folder`, `layout.load-folders-missing-folder`,
//! `layout.wrong-root`, `layout.unparsable-file` and `layout.texture-missing`.

use std::collections::BTreeSet;

use rimstudio_core::paths;
use rimstudio_design::plan::{LayoutProfile, ProjectLayout, WeaponCategory};
use rimstudio_ipc_types::diagnostic::SeverityDto;
use rimstudio_ipc_types::project::{LayoutFixDto, LayoutFixKindDto, LayoutIssueDto, NodeRoleDto};

use super::scan::{FileFacts, Scan, WeaponFact, in_active_content};
use crate::shared::projectfs::ProjectView;

/// The most issues one check reports.
pub const MAX_ISSUES: usize = 500;
/// The texture file extensions the game reads that the check accepts.
const TEXTURE_EXTENSIONS: [&str; 5] = ["png", "jpg", "jpeg", "dds", "tga"];

fn no_fix() -> LayoutFixDto {
    LayoutFixDto {
        kind: LayoutFixKindDto::None,
        summary: String::new(),
        automatic: false,
        targets: Vec::new(),
    }
}

fn issue(
    code: &str,
    severity: SeverityDto,
    path: &str,
    message: String,
    fix: LayoutFixDto,
) -> LayoutIssueDto {
    LayoutIssueDto {
        code: code.to_owned(),
        severity,
        path: path.to_owned(),
        message,
        fix,
    }
}

fn manual(kind: LayoutFixKindDto, summary: String, targets: Vec<String>) -> LayoutFixDto {
    LayoutFixDto {
        kind,
        summary,
        automatic: false,
        targets,
    }
}

/// The category a tech level text belongs to for a kind, as the designer chooses it.
fn expected_category(w: &WeaponFact) -> WeaponCategory {
    use rimstudio_design::model::{ItemKind, TechLevel};
    let tier = w.tech_level.as_deref().and_then(|t| {
        TechLevel::ALL
            .into_iter()
            .find(|l| l.xml_name().eq_ignore_ascii_case(t))
    });
    let kind = if w.ranged {
        ItemKind::Ranged
    } else {
        ItemKind::Melee
    };
    WeaponCategory::of(kind, tier)
}

/// The category named by the place of a weapon file under the weapon folder: the category folder of the
/// RimStudio layout, the category file of the game's own style.
fn category_of_place(rel: &str, layout: &ProjectLayout) -> Option<WeaponCategory> {
    let folder = layout.weapons_folder();
    let below = rel
        .get(..folder.len())
        .filter(|head| head.eq_ignore_ascii_case(&folder))
        .and_then(|_| rel.get(folder.len()..))?
        .strip_prefix('/')?;
    match layout.profile {
        LayoutProfile::Rimstudio => below
            .split_once('/')
            .and_then(|(dir, _)| WeaponCategory::parse(dir)),
        LayoutProfile::CoreStyle => below
            .rsplit_once('.')
            .and_then(|(stem, _)| WeaponCategory::parse(stem)),
        LayoutProfile::Flat => None,
    }
}

fn missing_standard_folders(view: &ProjectView, scan: &Scan, out: &mut Vec<LayoutIssueDto>) {
    let l = &view.layout;
    let mut wanted: Vec<String> = vec![l.in_version(&l.defs_dir)];
    if !l.weapons_dir.is_empty() {
        wanted.push(l.weapons_folder());
    }
    wanted.push(l.in_version(&format!("{}/{}", l.defs_dir, paths::DEFS_SOUNDS_DIR)));
    wanted.push(l.in_version(paths::PATCHES_DIR));
    wanted.push(l.in_version(paths::TEXTURES_DIR));
    wanted.push(l.in_version(paths::SOUNDS_DIR));
    let prefix = l.version_folder.as_ref().map(|v| format!("{v}/"));
    for folder in wanted {
        // the game also loads the project root next to a version folder or `Common`, so a standard folder
        // that lives at the root counts as present
        let at_root = prefix
            .as_deref()
            .and_then(|p| folder.strip_prefix(p))
            .is_some_and(|rel| scan.has_ci(rel));
        if scan.has_ci(&folder) || at_root {
            continue;
        }
        out.push(issue(
            "layout.missing-folder",
            SeverityDto::Info,
            &folder,
            format!("The standard folder {folder} does not exist."),
            LayoutFixDto {
                kind: LayoutFixKindDto::CreateFolder,
                summary: format!("Create the empty folder {folder}."),
                automatic: true,
                targets: vec![folder.clone()],
            },
        ));
    }
}

fn folder_case(view: &ProjectView, scan: &Scan, out: &mut Vec<LayoutIssueDto>) {
    const AREAS: [&str; 7] = [
        paths::ABOUT_DIR,
        paths::DEFS_DIR,
        paths::PATCHES_DIR,
        paths::TEXTURES_DIR,
        paths::SOUNDS_DIR,
        paths::LANGUAGES_DIR,
        paths::ASSEMBLIES_DIR,
    ];
    let prefix = view
        .layout
        .version_folder
        .as_ref()
        .map_or(String::new(), |v| format!("{v}/"));
    for e in scan.entries.iter().filter(|e| e.is_dir) {
        let Some(name) = e.rel.strip_prefix(&prefix).filter(|n| !n.contains('/')) else {
            continue;
        };
        if let Some(exact) = AREAS
            .iter()
            .find(|a| a.eq_ignore_ascii_case(name) && **a != name)
        {
            out.push(issue(
                "layout.folder-case",
                SeverityDto::Warning,
                &e.rel,
                format!(
                    "The folder is named {name}; the game looks for {exact} and does not find {name} on Linux and macOS."
                ),
                manual(
                    LayoutFixKindDto::MoveFile,
                    format!("Rename the folder to {exact}."),
                    vec![format!("{prefix}{exact}")],
                ),
            ));
        }
    }
}

fn weapon_files(view: &ProjectView, facts: &[FileFacts], out: &mut Vec<LayoutIssueDto>) {
    let l = &view.layout;
    let folder = l.weapons_folder();
    for f in facts.iter().filter(|f| !f.weapons.is_empty()) {
        if !in_active_content(&f.rel, l) {
            continue;
        }
        let names: Vec<&str> = f.weapons.iter().map(|w| w.def_name.as_str()).collect();
        let place = category_of_place(&f.rel, l);
        let inside = f.rel.len() > folder.len()
            && f.rel
                .get(..folder.len())
                .is_some_and(|h| h.eq_ignore_ascii_case(&folder));
        if !inside {
            if l.weapons_dir.is_empty() {
                continue;
            }
            let first = &f.weapons[0];
            let target = match l.profile {
                LayoutProfile::Flat => {
                    format!("{folder}/{}", f.rel.rsplit('/').next().unwrap_or(&f.rel))
                }
                _ => l.weapon_def_path(expected_category(first), &first.def_name),
            };
            out.push(issue(
                "layout.weapon-misplaced",
                SeverityDto::Warning,
                &f.rel,
                format!(
                    "The file defines {} outside the weapon folder {folder}.",
                    names.join(", ")
                ),
                manual(
                    LayoutFixKindDto::MoveFile,
                    format!("Move the file to {target} (RimStudio never moves it for you)."),
                    vec![target],
                ),
            ));
            continue;
        }
        let Some(cat) = place else { continue };
        for w in &f.weapons {
            let expected = expected_category(w);
            let kind_differs = expected.is_ranged() != cat.is_ranged();
            let tier_differs = !kind_differs
                && w.tech_level.is_some()
                && expected != cat
                && !matches!(
                    cat,
                    WeaponCategory::RangedMechanoid | WeaponCategory::RangedSpecial
                );
            if !(kind_differs || tier_differs) {
                continue;
            }
            let target = l.weapon_def_path(expected, &w.def_name);
            let (sev, what) = if kind_differs {
                (
                    SeverityDto::Warning,
                    if w.ranged {
                        "a ranged weapon in a melee category"
                    } else {
                        "a melee weapon in a ranged category"
                    },
                )
            } else {
                (
                    SeverityDto::Info,
                    "a weapon whose tech level fits another category",
                )
            };
            out.push(issue(
                "layout.def-wrong-category",
                sev,
                &f.rel,
                format!("{} is {what} ({}).", w.def_name, cat.name()),
                manual(
                    LayoutFixKindDto::MoveFile,
                    format!("Move the definition to {target}."),
                    vec![target],
                ),
            ));
        }
    }
}

fn combat_extended(view: &ProjectView, scan: &Scan, out: &mut Vec<LayoutIssueDto>) {
    let l = &view.layout;
    let ce_dir = l.ce_dir();
    if !view.requires_ce {
        for f in scan.facts.iter().filter(|f| f.ce_class) {
            let role = scan.entry_ci(&f.rel).map(|e| e.role);
            if matches!(role, Some(NodeRoleDto::CeCompat) | None) {
                continue;
            }
            let name = f.rel.rsplit('/').next().unwrap_or(&f.rel);
            let target = format!("{ce_dir}/{}/{name}", paths::PATCHES_DIR);
            out.push(issue(
                "layout.ce-outside-gate",
                SeverityDto::Warning,
                &f.rel,
                "The file uses Combat Extended classes outside the gated Combat Extended folder, so the game reports errors when Combat Extended is not active."
                    .to_owned(),
                manual(
                    LayoutFixKindDto::MoveFile,
                    format!(
                        "Move the file to {target} and gate that folder in LoadFolders.xml, or list Combat Extended as a required mod."
                    ),
                    vec![target],
                ),
            ));
        }
    }
    let has_files = scan
        .entries
        .iter()
        .any(|e| !e.is_dir && e.role == NodeRoleDto::CeCompat);
    if scan.ce_folder_exists && has_files && !view.gates_on(paths::CE_PACKAGE_ID) {
        let why = if view.load_folders.is_some() {
            "LoadFolders.xml has no entry for it that is conditional on Combat Extended"
        } else {
            "the project has no LoadFolders.xml, so the game never loads it"
        };
        out.push(issue(
            "layout.ce-folder-ungated",
            SeverityDto::Warning,
            &ce_dir,
            format!("The Combat Extended folder is not gated: {why}."),
            manual(
                LayoutFixKindDto::EditLoadFolders,
                format!(
                    "Add an entry for {ce_dir} to the block of your game version in LoadFolders.xml with IfModActive set to {}, or plan a Combat Extended patch in the designer, which adds it.",
                    paths::CE_PACKAGE_ID
                ),
                vec!["LoadFolders.xml".to_owned()],
            ),
        ));
    }
    if scan.ce_folder_exists && l.has_legacy_ce_folder() {
        out.push(issue(
            "layout.ce-legacy-folder",
            SeverityDto::Info,
            &ce_dir,
            format!(
                "The Combat Extended folder is {} and not {}. It works and stays where it is.",
                l.ce_folder,
                paths::CE_COMPAT_DIR
            ),
            no_fix(),
        ));
    }
}

fn load_folders_targets(view: &ProjectView, scan: &Scan, out: &mut Vec<LayoutIssueDto>) {
    let Some(file) = &view.load_folders else {
        return;
    };
    let mut seen = BTreeSet::new();
    for block in file.root.elements() {
        for li in block.children_named("li") {
            let text = li.text_content().replace('\\', "/");
            let entry = text.trim().trim_matches('/');
            if entry.is_empty() || entry.contains("..") || !seen.insert(entry.to_ascii_lowercase())
            {
                continue;
            }
            if !scan.has_ci(entry) {
                out.push(issue(
                    "layout.load-folders-missing-folder",
                    SeverityDto::Warning,
                    &file.rel,
                    format!(
                        "{} lists the folder {entry}, which does not exist.",
                        file.rel
                    ),
                    manual(
                        LayoutFixKindDto::EditLoadFolders,
                        format!("Create {entry} or remove the entry."),
                        vec![entry.to_owned()],
                    ),
                ));
            }
        }
    }
}

fn file_findings(scan: &Scan, out: &mut Vec<LayoutIssueDto>) {
    for f in &scan.facts {
        let role = scan.entry_ci(&f.rel).map(|e| e.role);
        if let Some(why) = &f.unparsable {
            out.push(issue(
                "layout.unparsable-file",
                SeverityDto::Warning,
                &f.rel,
                format!("The file is not valid XML: {why}"),
                no_fix(),
            ));
            continue;
        }
        let Some(tag) = f.root_tag.as_deref() else {
            continue;
        };
        let (expected, area) = match role {
            Some(NodeRoleDto::Defs | NodeRoleDto::DefsWeapons | NodeRoleDto::DefsSounds) => {
                ("Defs", "a Defs folder")
            }
            Some(NodeRoleDto::Patches | NodeRoleDto::CeCompat) => ("Patch", "a Patches folder"),
            _ => continue,
        };
        if tag != expected {
            out.push(issue(
                "layout.wrong-root",
                SeverityDto::Warning,
                &f.rel,
                format!(
                    "The file sits in {area} but its root element is {tag}, not {expected}; the game ignores it."
                ),
                manual(
                    LayoutFixKindDto::MoveFile,
                    "Move the file to the matching folder.".to_owned(),
                    Vec::new(),
                ),
            ));
        }
    }
}

fn textures(view: &ProjectView, scan: &Scan, out: &mut Vec<LayoutIssueDto>) {
    let l = &view.layout;
    // every texture file as a lower case path below its Textures folder, without the extension
    let mut have: BTreeSet<String> = BTreeSet::new();
    for e in scan
        .entries
        .iter()
        .filter(|e| !e.is_dir && e.role == NodeRoleDto::Textures)
    {
        let lower = e.rel.to_ascii_lowercase();
        let Some(pos) = lower.find("textures/") else {
            continue;
        };
        let below = &lower[pos + "textures/".len()..];
        if let Some((stem, ext)) = below.rsplit_once('.')
            && TEXTURE_EXTENSIONS.contains(&ext)
        {
            have.insert(stem.to_owned());
        }
    }
    for f in &scan.facts {
        if !in_active_content(&f.rel, l) {
            continue;
        }
        for w in &f.weapons {
            let Some(tex) = &w.tex_path else { continue };
            if have.contains(&tex.to_ascii_lowercase()) {
                continue;
            }
            let target = l.texture_file(tex);
            out.push(issue(
                "layout.texture-missing",
                SeverityDto::Info,
                &f.rel,
                format!(
                    "The texture {tex} of {} is not in the project. That is fine for a vanilla texture; for your own art the file goes to {target}.",
                    w.def_name
                ),
                manual(LayoutFixKindDto::AddFile, format!("Add the art at {target}."), vec![target]),
            ));
        }
    }
}

/// Runs the layout check over a scanned project.
#[must_use]
pub fn check(view: &ProjectView, scan: &Scan) -> Vec<LayoutIssueDto> {
    let mut out = Vec::new();
    if !scan.has_ci(&view.about_rel) {
        out.push(issue(
            "layout.missing-about",
            SeverityDto::Error,
            &view.about_rel,
            "The project has no About/About.xml; the game does not list it as a mod.".to_owned(),
            manual(
                LayoutFixKindDto::AddFile,
                "Create About/About.xml (the new project dialog writes a complete one).".to_owned(),
                vec![paths::ABOUT_XML.to_owned()],
            ),
        ));
    }
    missing_standard_folders(view, scan, &mut out);
    folder_case(view, scan, &mut out);
    weapon_files(view, &scan.facts, &mut out);
    combat_extended(view, scan, &mut out);
    load_folders_targets(view, scan, &mut out);
    file_findings(scan, &mut out);
    textures(view, scan, &mut out);
    out.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.path.cmp(&b.path))
            .then_with(|| a.code.cmp(&b.code))
            .then_with(|| a.message.cmp(&b.message))
    });
    out.truncate(MAX_ISSUES);
    out
}
