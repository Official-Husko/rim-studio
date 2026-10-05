//! Finding mod roots below a source (crate private).
//!
//! Discovery lists folders and nothing else. One [`probe_dir`] call reads a folder once, finds the
//! `About` folder (ignoring case), lists it, and notes `LoadFolders.xml`. Mods are found level by
//! level: the candidates of one level are probed in parallel, the folders without an `About.xml`
//! become the candidates of the next level, up to the scan depth of the source. Links are listed but
//! never followed below the first level, hidden folders are skipped, and every problem becomes a
//! diagnostic instead of an error.

use camino::{Utf8Path, Utf8PathBuf};
use rayon::prelude::*;
use rimstudio_core::diag::{Diagnostic, Severity};
use rimstudio_core::jobs::CancelToken;
use rimstudio_core::mods::SourceKind;
use rimstudio_core::paths;
use rimstudio_core::settings::FolderLayout;

use crate::error::codes;
use crate::sources::{FolderSpec, is_ignored_dir_name};

/// A child folder of a probed folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildDir {
    pub(crate) name: String,
    pub(crate) is_link: bool,
}

/// The files of the `About` folder that matter to the scanner.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AboutFiles {
    pub(crate) xml: Option<String>,
    pub(crate) published_id: Option<String>,
    pub(crate) icon: Option<String>,
    pub(crate) preview: Option<String>,
}

/// What one folder listing showed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct DirProbe {
    /// Child folders (links to folders included), sorted by name.
    pub(crate) dirs: Vec<ChildDir>,
    /// The name of the `About` folder as spelled on disk.
    pub(crate) about_dir: Option<String>,
    /// The files found inside it.
    pub(crate) about: AboutFiles,
    /// The name of the `LoadFolders.xml` file as spelled on disk.
    pub(crate) load_folders: Option<String>,
    /// Links that point nowhere (or loop), by name.
    pub(crate) dangling: Vec<String>,
    /// Entries whose names are not UTF-8.
    pub(crate) non_utf8: u64,
    /// Entries that could not be examined.
    pub(crate) entry_errors: u64,
    /// The error of the listing itself, when the folder could not be read.
    pub(crate) read_error: Option<String>,
}

impl DirProbe {
    /// True when the folder holds `About/About.xml`.
    pub(crate) fn is_mod(&self) -> bool {
        self.about.xml.is_some()
    }
}

/// Picks the best of several names that are equal ignoring case: the exact spelling when present,
/// otherwise the smallest name (so the choice does not depend on listing order).
fn prefer(current: &mut Option<String>, exact: &str, name: &str) {
    match current {
        None => *current = Some(name.to_owned()),
        Some(existing) => {
            if existing == exact {
                return;
            }
            if name == exact || name < existing.as_str() {
                *current = Some(name.to_owned());
            }
        }
    }
}

fn probe_about_dir(dir: &Utf8Path, probe: &mut DirProbe) {
    let Some(about_dir) = probe.about_dir.clone() else {
        return;
    };
    let Ok(rd) = fs_err::read_dir(dir.join(&about_dir)) else {
        return;
    };
    for entry in rd.flatten() {
        let Ok(name) = entry.file_name().into_string() else {
            probe.non_utf8 += 1;
            continue;
        };
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            continue;
        }
        if name.eq_ignore_ascii_case(paths::ABOUT_XML_NAME) {
            prefer(&mut probe.about.xml, paths::ABOUT_XML_NAME, &name);
        } else if name.eq_ignore_ascii_case("PublishedFileId.txt") {
            prefer(&mut probe.about.published_id, "PublishedFileId.txt", &name);
        } else if name.eq_ignore_ascii_case("ModIcon.png") {
            prefer(&mut probe.about.icon, "ModIcon.png", &name);
        } else if name.eq_ignore_ascii_case("Preview.png") {
            prefer(&mut probe.about.preview, "Preview.png", &name);
        }
    }
}

/// Lists `dir` once and, when it has an `About` folder, lists that too.
pub(crate) fn probe_dir(dir: &Utf8Path) -> DirProbe {
    let mut probe = DirProbe::default();
    let rd = match fs_err::read_dir(dir) {
        Ok(rd) => rd,
        Err(e) => {
            probe.read_error = Some(e.to_string());
            return probe;
        }
    };
    for entry in rd {
        let Ok(entry) = entry else {
            probe.entry_errors += 1;
            continue;
        };
        let Ok(name) = entry.file_name().into_string() else {
            probe.non_utf8 += 1;
            continue;
        };
        let Ok(ft) = entry.file_type() else {
            probe.entry_errors += 1;
            continue;
        };
        if ft.is_dir() {
            if name.eq_ignore_ascii_case(paths::ABOUT_DIR) {
                prefer(&mut probe.about_dir, paths::ABOUT_DIR, &name);
            }
            probe.dirs.push(ChildDir {
                name,
                is_link: false,
            });
        } else if ft.is_symlink() {
            match fs_err::metadata(dir.join(&name)) {
                Ok(m) if m.is_dir() => {
                    if name.eq_ignore_ascii_case(paths::ABOUT_DIR) {
                        prefer(&mut probe.about_dir, paths::ABOUT_DIR, &name);
                    }
                    probe.dirs.push(ChildDir {
                        name,
                        is_link: true,
                    });
                }
                Ok(_) => {
                    if name.eq_ignore_ascii_case(paths::LOAD_FOLDERS_XML) {
                        prefer(&mut probe.load_folders, paths::LOAD_FOLDERS_XML, &name);
                    }
                }
                Err(_) => probe.dangling.push(name),
            }
        } else if ft.is_file() && name.eq_ignore_ascii_case(paths::LOAD_FOLDERS_XML) {
            prefer(&mut probe.load_folders, paths::LOAD_FOLDERS_XML, &name);
        }
    }
    probe.dirs.sort_by(|a, b| a.name.cmp(&b.name));
    probe.dangling.sort();
    probe_about_dir(dir, &mut probe);
    probe
}

/// A mod root found by discovery.
#[derive(Debug, Clone)]
pub(crate) struct Found {
    /// The mod root.
    pub(crate) path: Utf8PathBuf,
    /// The path relative to the source folder (empty when the source is the mod).
    pub(crate) rel: Utf8PathBuf,
    /// The folder name.
    pub(crate) folder_name: String,
    /// True when the root is a link to a folder.
    pub(crate) is_link: bool,
    /// What the listing showed.
    pub(crate) probe: DirProbe,
}

/// A folder waiting to be probed.
struct Candidate {
    path: Utf8PathBuf,
    rel: Utf8PathBuf,
    name: String,
    is_link: bool,
}

/// What discovery of one source produced.
#[derive(Debug, Default)]
pub(crate) struct Discovery {
    pub(crate) found: Vec<Found>,
    pub(crate) diagnostics: Vec<Diagnostic>,
    pub(crate) probed: usize,
    pub(crate) dir_errors: u64,
    pub(crate) non_utf8: u64,
}

fn arg_path(d: Diagnostic, path: &Utf8Path) -> Diagnostic {
    d.with_arg("path", path.as_str())
}

/// Reports what a listing found besides mods (unreadable folder, bad names, dangling links).
fn note_probe(probe: &DirProbe, path: &Utf8Path, out: &mut Discovery) {
    if let Some(err) = &probe.read_error {
        out.dir_errors += 1;
        out.diagnostics.push(
            arg_path(
                Diagnostic::new(
                    codes::DIR_UNREADABLE,
                    Severity::Warning,
                    "A folder could not be read.",
                ),
                path,
            )
            .with_arg("error", err.clone()),
        );
    }
    out.dir_errors += probe.entry_errors;
    out.non_utf8 += probe.non_utf8;
    if probe.non_utf8 > 0 {
        out.diagnostics.push(
            arg_path(
                Diagnostic::new(
                    codes::NON_UTF8_PATH,
                    Severity::Info,
                    "Names that are not valid UTF-8 were skipped.",
                ),
                path,
            )
            .with_arg("count", probe.non_utf8.to_string()),
        );
    }
    for name in &probe.dangling {
        out.diagnostics.push(arg_path(
            Diagnostic::new(
                codes::DANGLING_LINK,
                Severity::Info,
                "A link points to a missing folder.",
            ),
            &path.join(name),
        ));
    }
}

/// Finds the mod roots of one source.
///
/// `builtin` sources (game `Data`, game `Mods`, Workshop) always use one level of children. Custom
/// sources follow their [`FolderSpec`]: `Auto` treats the folder itself as a mod when it has an
/// `About.xml` and otherwise searches `depth` levels, `ModsRoot` never treats the folder as a mod,
/// and `SingleMod` looks only at the folder itself. `cancel` is checked between levels.
pub(crate) fn discover_source(
    root: &Utf8Path,
    kind: SourceKind,
    spec: FolderSpec,
    cancel: &CancelToken,
) -> Discovery {
    let mut out = Discovery::default();
    let builtin = kind != SourceKind::Custom;
    let (layout, depth) = if builtin {
        (FolderLayout::ModsRoot, 1)
    } else {
        (spec.layout, usize::from(spec.clamped_depth()))
    };

    let root_probe = probe_dir(root);
    out.probed += 1;
    note_probe(&root_probe, root, &mut out);
    if root_probe.read_error.is_some() {
        return out;
    }
    let folder_name = root.file_name().unwrap_or("").to_owned();

    if layout != FolderLayout::ModsRoot && root_probe.is_mod() {
        out.found.push(Found {
            path: root.to_owned(),
            rel: Utf8PathBuf::new(),
            folder_name,
            is_link: false,
            probe: root_probe,
        });
        return out;
    }
    if layout == FolderLayout::SingleMod {
        out.diagnostics.push(arg_path(
            Diagnostic::new(
                codes::NO_ABOUT,
                Severity::Warning,
                "The folder has no About/About.xml.",
            ),
            root,
        ));
        return out;
    }

    let mut candidates: Vec<Candidate> =
        children_of(&root_probe, root, Utf8Path::new(""), 1, &mut out);
    for level in 1..=depth {
        if candidates.is_empty() || cancel.is_cancelled() {
            break;
        }
        let probes: Vec<DirProbe> = candidates.par_iter().map(|c| probe_dir(&c.path)).collect();
        out.probed += probes.len();
        let mut next: Vec<Candidate> = Vec::new();
        for (cand, mut probe) in candidates.into_iter().zip(probes) {
            if cand.is_link {
                // What a link shows is reported where it really lives.
                probe.dangling.clear();
            }
            note_probe(&probe, &cand.path, &mut out);
            if probe.read_error.is_some() {
                continue;
            }
            if probe.is_mod() {
                out.found.push(Found {
                    path: cand.path,
                    rel: cand.rel,
                    folder_name: cand.name,
                    is_link: cand.is_link,
                    probe,
                });
                continue;
            }
            if probe.about_dir.is_some() {
                out.diagnostics.push(arg_path(
                    Diagnostic::new(
                        codes::NO_ABOUT,
                        Severity::Warning,
                        "The About folder has no About.xml.",
                    ),
                    &cand.path,
                ));
                continue;
            }
            if builtin && level == 1 {
                out.diagnostics.push(arg_path(
                    Diagnostic::new(
                        codes::NO_ABOUT,
                        Severity::Warning,
                        "The folder has no About folder, so it is not a mod.",
                    ),
                    &cand.path,
                ));
                continue;
            }
            if cand.is_link {
                // A link that is not a mod is never entered: it could point back into the tree.
                out.diagnostics.push(arg_path(
                    Diagnostic::new(
                        codes::LINK_SKIPPED,
                        Severity::Info,
                        "A link to a folder that is not a mod was not followed.",
                    ),
                    &cand.path,
                ));
                continue;
            }
            if level < depth {
                next.extend(children_of(
                    &probe,
                    &cand.path,
                    &cand.rel,
                    level + 1,
                    &mut out,
                ));
            }
        }
        candidates = next;
    }
    out
}

/// The candidates one level down. Links are accepted at the first level (a link farm and hand made
/// links are normal there) and skipped, with a diagnostic, below it.
fn children_of(
    probe: &DirProbe,
    path: &Utf8Path,
    rel: &Utf8Path,
    level: usize,
    out: &mut Discovery,
) -> Vec<Candidate> {
    let mut list = Vec::new();
    for child in &probe.dirs {
        if is_ignored_dir_name(&child.name) || child.name.eq_ignore_ascii_case(paths::ABOUT_DIR) {
            continue;
        }
        let child_path = path.join(&child.name);
        if child.is_link && level > 1 {
            out.diagnostics.push(arg_path(
                Diagnostic::new(
                    codes::LINK_SKIPPED,
                    Severity::Info,
                    "A link below the source root was not followed.",
                ),
                &child_path,
            ));
            continue;
        }
        list.push(Candidate {
            path: child_path,
            rel: rel.join(&child.name),
            name: child.name.clone(),
            is_link: child.is_link,
        });
    }
    list
}
