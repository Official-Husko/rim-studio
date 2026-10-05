//! The undo journal of a layout fix and the hashing it relies on.
//!
//! Before an apply moves anything it writes a journal to the data root
//! (`project-fix-journal/<projectId>/<applyId>.json`): every move with the hash of what is moved, every
//! edited file with its previous text and the hashes before and after, every created folder. The journal is
//! rewritten after each item. An undo trusts the journal for what to reverse and trusts the disk for
//! whether it can: a move is reversed only when the destination still holds exactly the recorded bytes.
//!
//! A journal carries a checksum over its own content, so a damaged or hand edited file is refused. The
//! checksum is not a secret: a journal is also checked against the project (its root, every path through the
//! same path rules as the apply, the hashes against the disk), so a forged journal can still only move
//! content that matches what it records, inside the project.

use std::io::ErrorKind;
use std::path::Path;

use camino::Utf8PathBuf;
use rimstudio_io::atomic::atomic_write;
use rimstudio_io::guard::to_safe_component;
use serde::{Deserialize, Serialize};

use crate::error::{ToolkitError, ToolkitResult};
use crate::shared::env::ProjectEnv;

/// The folder below the data root that holds the journals.
pub const JOURNAL_FOLDER: &str = "project-fix-journal";
/// The journal schema this build reads and writes.
pub const JOURNAL_SCHEMA: u32 = 1;
/// The largest journal file read, in bytes.
pub const MAX_JOURNAL_BYTES: u64 = 8 * 1024 * 1024;
/// The deepest folder level the tree hash descends to.
const MAX_HASH_DEPTH: usize = 32;
/// The most entries the tree hash visits.
const MAX_HASH_ENTRIES: usize = 200_000;

/// What an item did, as the journal records it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JournalKind {
    /// A file was moved.
    MoveFile,
    /// A folder was moved or renamed.
    MoveFolder,
    /// An empty folder was created.
    CreateFolder,
    /// `LoadFolders.xml` was edited.
    EditLoadFolders,
    /// `LoadFolders.xml` was created.
    CreateLoadFolders,
}

/// Where an item stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JournalStatus {
    /// Planned; not known to be done (a crash can leave an item here after it was done).
    Pending,
    /// Done.
    Done,
    /// Tried and failed; nothing of it is left.
    Failed,
    /// Not carried out.
    Skipped,
}

/// Where the whole apply stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum JournalState {
    /// Written before the first change; the apply may still be running or may have been interrupted.
    Started,
    /// Every selected item was handled.
    Complete,
    /// The job was cancelled between items.
    Cancelled,
    /// The apply was undone.
    Undone,
}

/// One item of a journal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JournalItem {
    /// The plan item id.
    pub id: String,
    /// What it does.
    pub kind: JournalKind,
    /// The source path relative to the project root; empty for a creation.
    pub from: String,
    /// The destination (or the created or edited path) relative to the project root.
    pub to: String,
    /// Where it stands.
    pub status: JournalStatus,
    /// The hash of what is moved (a file's bytes, or the manifest of a folder), before the move.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
    /// The previous text of an edited `LoadFolders.xml`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_text: Option<String>,
    /// The hash of the previous text (an edit), absent for a created file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_hash: Option<String>,
    /// The hash of the text written (an edit or a creation).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after_hash: Option<String>,
    /// For a move: the topmost folder the move created on the way to its destination (absent when every
    /// parent folder existed). An undo removes the empty chain again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_top: Option<String>,
    /// True when the move crossed a volume and was copied.
    #[serde(default)]
    pub copied: bool,
    /// Why an item failed or was skipped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// An apply, as written to disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Journal {
    /// The schema version.
    pub schema: u32,
    /// The apply id (also the file name).
    pub apply_id: String,
    /// The project id.
    pub project_id: String,
    /// The project root at the time of the apply.
    pub project_root: String,
    /// The plan id that was applied.
    pub plan_id: String,
    /// When the apply started, Unix milliseconds.
    pub created_ms: u64,
    /// When the apply was undone, Unix milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub undone_ms: Option<u64>,
    /// Where the whole apply stands.
    pub state: JournalState,
    /// The items in the order they are carried out.
    pub items: Vec<JournalItem>,
    /// The checksum of the journal with this field empty.
    #[serde(default)]
    pub checksum: String,
}

impl Journal {
    /// The checksum of the content (the `checksum` field excluded).
    #[must_use]
    pub fn compute_checksum(&self) -> String {
        let mut copy = self.clone();
        copy.checksum = String::new();
        let bytes = serde_json::to_vec(&copy).unwrap_or_default();
        blake3::hash(&bytes).to_hex().to_string()
    }

    /// Items that are or may be done: the ones an undo looks at.
    pub fn candidates(&self) -> impl Iterator<Item = &JournalItem> {
        self.items
            .iter()
            .filter(|i| !matches!(i.status, JournalStatus::Skipped))
    }
}

/// True for an apply id this build mints: lower case letters, digits and hyphens, at most 64 characters.
#[must_use]
pub fn is_valid_apply_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

/// The folder of the journals of a project.
#[must_use]
pub fn journal_dir(env: &ProjectEnv, project_id: &str) -> Utf8PathBuf {
    env.roots()
        .data
        .join(JOURNAL_FOLDER)
        .join(to_safe_component(project_id))
}

/// The path of one journal.
#[must_use]
pub fn journal_path(env: &ProjectEnv, project_id: &str, apply_id: &str) -> Utf8PathBuf {
    journal_dir(env, project_id).join(format!("{apply_id}.json"))
}

/// Writes (replaces) a journal atomically, with its checksum.
///
/// # Errors
///
/// [`ToolkitError::Store`] when the file cannot be written.
pub fn write_journal(env: &ProjectEnv, journal: &Journal) -> ToolkitResult<()> {
    let mut sealed = journal.clone();
    sealed.checksum = sealed.compute_checksum();
    let mut text = serde_json::to_string_pretty(&sealed)
        .map_err(|e| ToolkitError::internal(format!("journal cannot be serialised: {e}")))?;
    text.push('\n');
    let path = journal_path(env, &journal.project_id, &journal.apply_id);
    atomic_write(&path, text.as_bytes())?;
    Ok(())
}

/// Reads a journal and checks its checksum, schema and identity.
///
/// # Errors
///
/// [`ToolkitError::FixNotFound`] when there is no such file, [`ToolkitError::FixJournalDamaged`] when it is
/// too large, not JSON of the right shape, newer than this build, for another project or apply, or its
/// checksum does not hold.
pub fn read_journal(env: &ProjectEnv, project_id: &str, apply_id: &str) -> ToolkitResult<Journal> {
    if !is_valid_apply_id(apply_id) {
        return Err(ToolkitError::FixNotFound {
            id: apply_id.chars().take(80).collect(),
        });
    }
    let path = journal_path(env, project_id, apply_id);
    let damaged = |reason: String| ToolkitError::FixJournalDamaged {
        id: apply_id.to_owned(),
        reason,
    };
    let meta = match std::fs::metadata(path.as_std_path()) {
        Ok(m) => m,
        Err(e) if e.kind() == ErrorKind::NotFound => {
            return Err(ToolkitError::FixNotFound {
                id: apply_id.to_owned(),
            });
        }
        Err(e) => return Err(damaged(e.to_string())),
    };
    if meta.len() > MAX_JOURNAL_BYTES {
        return Err(damaged("the file is too large".to_owned()));
    }
    let bytes = std::fs::read(path.as_std_path()).map_err(|e| damaged(e.to_string()))?;
    let journal: Journal =
        serde_json::from_slice(&bytes).map_err(|e| damaged(format!("not a journal: {e}")))?;
    if journal.schema > JOURNAL_SCHEMA {
        return Err(damaged(format!(
            "written by a newer version (schema {})",
            journal.schema
        )));
    }
    if journal.apply_id != apply_id || journal.project_id != project_id {
        return Err(damaged(
            "the journal belongs to another apply or project".to_owned(),
        ));
    }
    if journal.checksum != journal.compute_checksum() {
        return Err(damaged(
            "the checksum does not hold; the file was changed after it was written".to_owned(),
        ));
    }
    Ok(journal)
}

/// The apply ids of a project's journals, sorted.
#[must_use]
pub fn list_apply_ids(env: &ProjectEnv, project_id: &str) -> Vec<String> {
    let Ok(read) = std::fs::read_dir(journal_dir(env, project_id).as_std_path()) else {
        return Vec::new();
    };
    let mut ids: Vec<String> = read
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let id = name.strip_suffix(".json")?;
            is_valid_apply_id(id).then(|| id.to_owned())
        })
        .collect();
    ids.sort();
    ids
}

/// The hash of a text.
#[must_use]
pub fn hash_text(text: &str) -> String {
    blake3::hash(text.as_bytes()).to_hex().to_string()
}

/// The hash of a file or, for a folder, of its manifest (every folder and file below it, by relative path,
/// with the hash of each file). A link anywhere in the tree is an error: it is never part of a move.
///
/// # Errors
///
/// An [`std::io::Error`] for a missing path, a link, an unreadable entry, a tree that is too deep or too
/// large, or a name that is not UTF-8.
pub fn hash_tree(path: &Path) -> std::io::Result<String> {
    let meta = std::fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(std::io::Error::other("a link"));
    }
    if !meta.is_dir() {
        return hash_file(path);
    }
    let mut lines = Vec::new();
    let mut budget = MAX_HASH_ENTRIES;
    manifest(path, "", 0, &mut budget, &mut lines)?;
    lines.sort();
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"folder\0");
    for line in &lines {
        hasher.update(line.as_bytes());
        hasher.update(b"\n");
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn hash_file(path: &Path) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(hasher.finalize().to_hex().to_string())
}

fn manifest(
    dir: &Path,
    rel: &str,
    depth: usize,
    budget: &mut usize,
    out: &mut Vec<String>,
) -> std::io::Result<()> {
    if depth > MAX_HASH_DEPTH {
        return Err(std::io::Error::other("the folder is nested too deep"));
    }
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        *budget = budget
            .checked_sub(1)
            .ok_or_else(|| std::io::Error::other("the folder holds too many entries"))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| std::io::Error::other("a name is not UTF-8"))?;
        let child = if rel.is_empty() {
            name
        } else {
            format!("{rel}/{name}")
        };
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(std::io::Error::other("a link inside the folder"));
        }
        if kind.is_dir() {
            out.push(format!("d:{child}"));
            manifest(&entry.path(), &child, depth + 1, budget, out)?;
        } else {
            out.push(format!("f:{child}:{}", hash_file(&entry.path())?));
        }
    }
    Ok(())
}

/// True when an entry with exactly this name exists next to its parent (the folder listing is used, so a
/// volume that ignores letter case does not report a case twin as the entry itself). Links count as present.
#[must_use]
pub fn exact_entry_exists(path: &Path) -> bool {
    let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
        return false;
    };
    let Ok(read) = std::fs::read_dir(parent) else {
        return false;
    };
    read.flatten().any(|e| e.file_name() == name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_ids_are_plain() {
        assert!(is_valid_apply_id("fix-1700000000000-ab12cd34"));
        for bad in ["", "A", "a/b", "..", "a.json", &"a".repeat(65)] {
            assert!(!is_valid_apply_id(bad), "{bad}");
        }
    }

    #[test]
    fn a_tree_hash_changes_with_any_byte_or_name() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        std::fs::create_dir_all(root.join("a/b")).unwrap();
        std::fs::write(root.join("a/b/f.txt"), "one").unwrap();
        let first = hash_tree(&root.join("a")).unwrap();
        assert_eq!(first, hash_tree(&root.join("a")).unwrap());
        std::fs::write(root.join("a/b/f.txt"), "two").unwrap();
        let second = hash_tree(&root.join("a")).unwrap();
        assert_ne!(first, second);
        std::fs::rename(root.join("a/b/f.txt"), root.join("a/b/g.txt")).unwrap();
        assert_ne!(second, hash_tree(&root.join("a")).unwrap());
        // a file and a folder never collide, and the same bytes in two places hash alike
        std::fs::write(root.join("x.txt"), "two").unwrap();
        std::fs::write(root.join("y.txt"), "two").unwrap();
        assert_eq!(
            hash_tree(&root.join("x.txt")).unwrap(),
            hash_tree(&root.join("y.txt")).unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_link_in_a_tree_is_an_error() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path();
        std::fs::create_dir_all(root.join("a")).unwrap();
        std::os::unix::fs::symlink(root, root.join("a/loop")).unwrap();
        assert!(hash_tree(&root.join("a")).is_err());
        assert!(hash_tree(&root.join("a/loop")).is_err());
    }

    #[test]
    fn the_checksum_covers_everything_but_itself() {
        let mut j = Journal {
            schema: 1,
            apply_id: "fix-1-aa".into(),
            project_id: "p-1".into(),
            project_root: "/x".into(),
            plan_id: "abc".into(),
            created_ms: 1,
            undone_ms: None,
            state: JournalState::Started,
            items: vec![],
            checksum: String::new(),
        };
        let a = j.compute_checksum();
        j.checksum = "anything".into();
        assert_eq!(a, j.compute_checksum());
        j.plan_id = "abd".into();
        assert_ne!(a, j.compute_checksum());
    }
}
