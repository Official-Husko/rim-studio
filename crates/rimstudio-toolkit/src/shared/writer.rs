//! The guarded writer: the only way the toolkit puts bytes on disk.
//!
//! A [`GuardedWriter`] belongs to one project root. Every path it accepts is a safe relative path (no
//! `..`, no drive, no absolute start), resolves inside the project root without a link that leaves it
//! ([`RootGuard`]) and lies outside every protected folder (the game install, the game config folder, the
//! folders of the reference mods) and outside the optional [`GameWriteFence`]. A replaced file is first copied
//! into the backup folder, the new bytes go through an atomic temp then rename write, and the file is read
//! back and compared before the call returns (I-05, IT-004, IT-058).
//!
//! Beyond the root guard the writer enforces, for every path, the rules of the write path review
//! (`docs/architecture/security-and-privacy.md`, section 19): every segment is a safe file name on Windows
//! too (no reserved device name, no trailing dot or space, no stream or wildcard character, at most 255
//! bytes), the whole relative path is at most [`MAX_REL_PATH_BYTES`] bytes, a segment may not differ only by
//! letter case from an entry that exists (so a case sensitive disk cannot grow a twin folder that breaks on
//! Windows and macOS), the target file itself may not be a link (a write would replace the link), a read
//! only file is never replaced, and the project folder may not be a link or live in a Steam library. Input
//! and output go through the canonical path, and the checks run again after the parent folders exist and
//! right before the bytes are written, so a link planted in between is seen.
//!
//! Backups never go inside the mod folder: a Workshop upload would ship them. The caller gives a backup
//! root (the app data folder) and each file gets its own sub folder there.

use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use rimstudio_core::ports::Clock;
use rimstudio_io::atomic::{atomic_write, create_dir_all};
use rimstudio_io::backup::{BackupPolicy, rotate};
use rimstudio_io::error::StoreError;
use rimstudio_io::fence::GameWriteFence;
use rimstudio_io::guard::{
    FOLD_CASE_PATHS, RootGuard, lexical_clean, path_starts_with, resolve_path, sanitize_name,
    to_safe_component,
};

use crate::error::{ToolkitError, ToolkitResult};

mod copy;
mod remove;

pub use copy::CopyReport;
pub use remove::RemoveReport;

/// The longest relative path (bytes) the writer accepts. Windows tools stop at 260 characters for the whole
/// path, so a mod path that is much longer cannot be used by every player.
pub const MAX_REL_PATH_BYTES: usize = 200;

/// What one write did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteReport {
    /// The absolute path written.
    pub path: Utf8PathBuf,
    /// The path relative to the project root, with `/` separators.
    pub rel: String,
    /// Bytes written.
    pub bytes: usize,
    /// True when the file existed before and was replaced.
    pub replaced: bool,
    /// The backup copy of the replaced file, when one was made.
    pub backup: Option<Utf8PathBuf>,
    /// True when the file was read back and equals what was written.
    pub verified: bool,
}

/// Writes files of one project, with every path checked (see the module documentation).
#[derive(Clone)]
pub struct GuardedWriter {
    root: Utf8PathBuf,
    real_root: Utf8PathBuf,
    guard: Arc<RootGuard>,
    protected: Vec<Utf8PathBuf>,
    fence: Option<Arc<GameWriteFence>>,
    backup_root: Utf8PathBuf,
    clock: Arc<dyn Clock>,
}

impl std::fmt::Debug for GuardedWriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuardedWriter")
            .field("root", &self.root)
            .field("protected", &self.protected)
            .finish_non_exhaustive()
    }
}

fn refused(path: &str, reason: impl Into<String>) -> ToolkitError {
    ToolkitError::PathRefused {
        path: path.chars().take(200).collect(),
        reason: reason.into(),
    }
}

impl GuardedWriter {
    /// A writer for the project at `root` (absolute).
    ///
    /// `protected` lists folders that are never written (they are resolved through links when they exist);
    /// `backup_root` is the folder backups go to; `fence` is the optional game write fence of the app.
    ///
    /// # Errors
    ///
    /// [`ToolkitError::PathRefused`] when the root is not an absolute, clean path or lies inside a protected
    /// folder.
    pub fn new(
        root: &Utf8Path,
        protected: &[Utf8PathBuf],
        backup_root: Utf8PathBuf,
        fence: Option<Arc<GameWriteFence>>,
        clock: Arc<dyn Clock>,
    ) -> ToolkitResult<Self> {
        let root = lexical_clean(root).map_err(|e| refused(root.as_str(), e.to_string()))?;
        if std::fs::symlink_metadata(root.as_std_path()).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(refused(
                root.as_str(),
                "the project folder is a link; open the real folder instead",
            ));
        }
        let guard =
            RootGuard::new([root.clone()]).map_err(|e| refused(root.as_str(), e.to_string()))?;
        let protected: Vec<Utf8PathBuf> = protected
            .iter()
            .map(|p| {
                resolve_path(p)
                    .or_else(|_| lexical_clean(p))
                    .unwrap_or_else(|_| p.clone())
            })
            .collect();
        let real_root = resolve_path(&root).map_err(|e| refused(root.as_str(), e.to_string()))?;
        if protected
            .iter()
            .any(|p| path_starts_with(&real_root, p, FOLD_CASE_PATHS))
        {
            return Err(refused(
                root.as_str(),
                "the project folder is inside a protected game folder",
            ));
        }
        if real_root
            .components()
            .any(|c| c.as_str().eq_ignore_ascii_case("steamapps"))
        {
            return Err(refused(
                root.as_str(),
                "the project folder is inside a Steam library folder",
            ));
        }
        Ok(Self {
            root,
            real_root,
            guard: Arc::new(guard),
            protected,
            fence,
            backup_root,
            clock,
        })
    }

    /// The project root.
    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    /// The absolute path of a relative project path, after every check.
    ///
    /// # Errors
    ///
    /// [`ToolkitError::PathRefused`] when the path is not a safe relative path (see the module
    /// documentation), leaves the project root, is inside a protected folder or is refused by the fence.
    pub fn path_of(&self, rel: &str) -> ToolkitResult<Utf8PathBuf> {
        Ok(self.checked(rel)?.abs)
    }

    fn checked(&self, rel: &str) -> ToolkitResult<Checked> {
        check_relative(rel)?;
        let abs = self.root.join(rel);
        let validated = self
            .guard
            .validate(&abs)
            .map_err(|e| refused(rel, e.to_string()))?;
        if self
            .protected
            .iter()
            .any(|p| path_starts_with(&validated.path, p, FOLD_CASE_PATHS))
        {
            return Err(refused(rel, "inside a protected game folder"));
        }
        if let Some(fence) = &self.fence {
            fence
                .check_project_write(&self.guard, &abs)
                .map_err(|e| refused(rel, e.to_string()))?;
        }
        if let Some(twin) = case_twin(&self.real_root, rel) {
            return Err(refused(
                rel,
                format!("differs only by letter case from the existing entry {twin}"),
            ));
        }
        if std::fs::symlink_metadata(abs.as_std_path()).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(refused(
                rel,
                "the file is a link; writing would replace the link",
            ));
        }
        Ok(Checked {
            abs,
            io: validated.path,
        })
    }

    /// The text of a project file, `None` when it does not exist.
    ///
    /// # Errors
    ///
    /// A path refusal, or [`ToolkitError::Store`] when the file cannot be read as UTF-8 text.
    pub fn read_text(&self, rel: &str) -> ToolkitResult<Option<String>> {
        let abs = self.checked(rel)?.io;
        match fs_read(&abs) {
            Ok(text) => Ok(Some(text)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(StoreError::io("read", &abs, e).into()),
        }
    }

    /// The first `max_bytes` bytes of a project file, `None` when it does not exist. The path goes through the
    /// same checks as a write (a safe relative path, inside the root, no link out of it, outside the
    /// protected folders), and only a regular file is read.
    ///
    /// # Errors
    ///
    /// A path refusal (also for a folder), or [`ToolkitError::Store`] when the file cannot be read.
    pub fn read_limited(&self, rel: &str, max_bytes: usize) -> ToolkitResult<Option<LimitedRead>> {
        use std::io::Read as _;
        let abs = self.checked(rel)?.io;
        let meta = match std::fs::metadata(abs.as_std_path()) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(StoreError::io("read", &abs, e).into()),
        };
        if !meta.is_file() {
            return Err(refused(rel, "not a regular file"));
        }
        let file = std::fs::File::open(abs.as_std_path())
            .map_err(|e| ToolkitError::from(StoreError::io("read", &abs, e)))?;
        let mut bytes = Vec::new();
        let limit = u64::try_from(max_bytes).unwrap_or(u64::MAX);
        file.take(limit)
            .read_to_end(&mut bytes)
            .map_err(|e| ToolkitError::from(StoreError::io("read", &abs, e)))?;
        let total = meta.len();
        Ok(Some(LimitedRead {
            truncated: total > u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            bytes,
            total,
        }))
    }

    /// True when the project file exists.
    ///
    /// # Errors
    ///
    /// A path refusal.
    pub fn exists(&self, rel: &str) -> ToolkitResult<bool> {
        Ok(self.checked(rel)?.io.as_std_path().exists())
    }

    /// Creates a folder (and its parents) inside the project.
    ///
    /// # Errors
    ///
    /// A path refusal or [`ToolkitError::Store`].
    pub fn make_dir(&self, rel: &str) -> ToolkitResult<()> {
        let checked = self.checked(rel)?;
        create_dir_all(&checked.io)?;
        // a link planted while the folders were created is seen here, before anything is written
        self.checked(rel)?;
        Ok(())
    }

    fn backup_policy(&self, rel: &str) -> BackupPolicy {
        let parent = rel.rsplit_once('/').map_or("", |(dir, _)| dir);
        let folder = if parent.is_empty() {
            "root".to_owned()
        } else {
            to_safe_component(&parent.replace('/', "__"))
        };
        BackupPolicy::user_data().with_root(self.backup_root.join(folder))
    }

    /// Writes `text` to a project file. A replaced file is backed up first when `backup` is true; the new
    /// content is written atomically and read back.
    ///
    /// # Errors
    ///
    /// A path refusal, or [`ToolkitError::Store`] when the backup, the write or the read back fails or the
    /// read back differs.
    pub fn write(&self, rel: &str, text: &str, backup: bool) -> ToolkitResult<WriteReport> {
        self.write_expecting(rel, text, backup, Expect::Any)
    }

    /// Checks everything [`GuardedWriter::write_expecting`] checks before it writes, without writing: the
    /// path, the read only flag and the expected current content.
    ///
    /// # Errors
    ///
    /// A path refusal, or [`ToolkitError::ApplyRefused`] when the file is read only or does not hold the
    /// expected content.
    pub fn preflight(&self, rel: &str, expect: Expect<'_>) -> ToolkitResult<()> {
        let checked = self.checked(rel)?;
        self.check_writable(rel, &checked.io)?;
        self.check_expected(rel, &checked.io, expect)
    }

    fn check_writable(&self, rel: &str, io: &Utf8Path) -> ToolkitResult<()> {
        if let Ok(meta) = std::fs::metadata(io.as_std_path())
            && meta.permissions().readonly()
        {
            return Err(refused(
                rel,
                "the file is read only; clear the read only flag to let RimStudio change it",
            ));
        }
        Ok(())
    }

    fn check_expected(&self, rel: &str, io: &Utf8Path, expect: Expect<'_>) -> ToolkitResult<()> {
        let Expect::Previous(previous) = expect else {
            return Ok(());
        };
        let now = match std::fs::read(io.as_std_path()) {
            Ok(bytes) => Some(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(StoreError::io("read", io, e).into()),
        };
        let same = match (&now, previous) {
            (None, None) => true,
            (Some(bytes), Some(text)) => bytes == text.as_bytes(),
            _ => false,
        };
        if same {
            Ok(())
        } else {
            Err(ToolkitError::ApplyRefused {
                reason: format!("{rel} changed on disk since the plan was made; plan again"),
            })
        }
    }

    /// Like [`GuardedWriter::write`], and refuses when the file does not hold the content the caller
    /// planned against (`Expect::Previous`; `None` means the file must not exist). The comparison is made
    /// immediately before the bytes go down, so an edit made by another program in between is never
    /// overwritten.
    ///
    /// # Errors
    ///
    /// As [`GuardedWriter::write`], and [`ToolkitError::ApplyRefused`] for a read only file or content that
    /// differs from the expectation.
    pub fn write_expecting(
        &self,
        rel: &str,
        text: &str,
        backup: bool,
        expect: Expect<'_>,
    ) -> ToolkitResult<WriteReport> {
        let first = self.checked(rel)?;
        self.check_writable(rel, &first.io)?;
        self.check_expected(rel, &first.io, expect)?;
        if let Some(parent) = first.io.parent() {
            create_dir_all(parent)?;
        }
        // the folders exist now: check again, then act on the canonical path only
        let checked = self.checked(rel)?;
        let io = checked.io;
        let replaced = io.as_std_path().is_file();
        let mut backup_path = None;
        if replaced && backup {
            let report = rotate(&io, &self.backup_policy(rel), self.clock.as_ref())?;
            backup_path = report.created;
        }
        // last look before the bytes go down: the backup step took time
        let io = self.checked(rel)?.io;
        self.check_expected(rel, &io, expect)?;
        atomic_write(&io, text.as_bytes())?;
        let verified = match fs_read_bytes(&io) {
            Ok(back) => back == text.as_bytes(),
            Err(e) => return Err(StoreError::io("verify", &io, e).into()),
        };
        if !verified {
            return Err(StoreError::WriteVerifyFailed { path: io }.into());
        }
        Ok(WriteReport {
            path: checked.abs,
            rel: rel.to_owned(),
            bytes: text.len(),
            replaced,
            backup: backup_path,
            verified,
        })
    }
}

/// What a write expects to find in the file it replaces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expect<'a> {
    /// No expectation.
    Any,
    /// The file holds exactly this text, or does not exist when `None`.
    Previous(Option<&'a str>),
}

/// A path that passed every check: the lexical absolute path and the canonical path all input and output
/// uses.
struct Checked {
    abs: Utf8PathBuf,
    io: Utf8PathBuf,
}

/// Checks the text of a relative path: plain segments, each a safe file name, and not too long.
fn check_relative(rel: &str) -> ToolkitResult<()> {
    if !rimstudio_design::plan::is_safe_relative_path(rel) {
        return Err(refused(rel, "not a safe relative path"));
    }
    if rel.len() > MAX_REL_PATH_BYTES {
        return Err(refused(
            rel,
            format!("the path is longer than {MAX_REL_PATH_BYTES} bytes"),
        ));
    }
    for segment in rel.split('/') {
        if let Err(e) = sanitize_name(segment) {
            return Err(refused(rel, e.to_string()));
        }
    }
    Ok(())
}

/// The bytes of a project file read with a limit: see [`GuardedWriter::read_limited`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LimitedRead {
    /// At most the requested number of bytes from the start of the file.
    pub bytes: Vec<u8>,
    /// The size of the whole file.
    pub total: u64,
    /// True when the file is longer than `bytes`.
    pub truncated: bool,
}

/// The first existing entry along `rel` (below `root`) whose name equals a segment of `rel` except for
/// letter case. `None` when every segment either matches exactly or does not exist yet.
fn case_twin(root: &Utf8Path, rel: &str) -> Option<String> {
    let mut dir = root.to_path_buf();
    for segment in rel.split('/') {
        let entries = std::fs::read_dir(dir.as_std_path()).ok()?;
        let wanted = segment.to_lowercase();
        let mut exact = false;
        let mut twin = None;
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if name == segment {
                exact = true;
            } else if name.to_lowercase() == wanted {
                twin = Some(name.to_owned());
            }
        }
        if let Some(twin) = twin.filter(|_| !exact) {
            return Some(twin);
        }
        if !exact {
            return None;
        }
        dir = dir.join(segment);
    }
    None
}

fn fs_read(path: &Utf8Path) -> std::io::Result<String> {
    std::fs::read_to_string(path.as_std_path())
}

fn fs_read_bytes(path: &Utf8Path) -> std::io::Result<Vec<u8>> {
    std::fs::read(path.as_std_path())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rimstudio_testing::fakes::FakeClock;

    fn writer(root: &Utf8Path, protected: &[Utf8PathBuf]) -> (GuardedWriter, Utf8PathBuf) {
        let backups = root.parent().unwrap_or(root).join("backups-test");
        let w = GuardedWriter::new(
            root,
            protected,
            backups.clone(),
            None,
            Arc::new(FakeClock::new(FakeClock::DEFAULT_START_MS)),
        )
        .unwrap();
        (w, backups)
    }

    fn temp() -> (tempfile::TempDir, Utf8PathBuf) {
        let t = tempfile::tempdir().unwrap();
        let p = Utf8PathBuf::from_path_buf(t.path().to_path_buf()).unwrap();
        std::fs::create_dir_all(p.join("proj").as_std_path()).unwrap();
        (t, p)
    }

    #[test]
    fn writes_inside_the_root_and_reads_back() {
        let (_t, base) = temp();
        let (w, _) = writer(&base.join("proj"), &[]);
        let r = w.write("Defs/A.xml", "one", true).unwrap();
        assert!(r.verified && !r.replaced && r.backup.is_none());
        assert_eq!(w.read_text("Defs/A.xml").unwrap().as_deref(), Some("one"));
        assert_eq!(w.read_text("Defs/none.xml").unwrap(), None);
    }

    #[test]
    fn a_replaced_file_is_backed_up_outside_the_project() {
        let (_t, base) = temp();
        let (w, backups) = writer(&base.join("proj"), &[]);
        w.write("Defs/A.xml", "one", true).unwrap();
        let r = w.write("Defs/A.xml", "two", true).unwrap();
        let backup = r.backup.unwrap();
        assert!(backup.starts_with(&backups));
        assert_eq!(
            std::fs::read_to_string(backup.as_std_path()).unwrap(),
            "one"
        );
        assert!(!base.join("proj/Defs/backups").as_std_path().exists());
    }

    #[test]
    fn unsafe_paths_are_refused() {
        let (_t, base) = temp();
        let (w, _) = writer(&base.join("proj"), &[]);
        for bad in ["../x.xml", "/etc/x", "a/../../x", "C:/x", ""] {
            assert!(
                matches!(
                    w.write(bad, "x", false),
                    Err(ToolkitError::PathRefused { .. })
                ),
                "{bad}"
            );
        }
        assert!(!base.join("x.xml").as_std_path().exists());
    }

    #[test]
    fn a_project_inside_a_protected_folder_cannot_be_written() {
        let (_t, base) = temp();
        let game = base.join("game");
        std::fs::create_dir_all(game.join("Mods/p").as_std_path()).unwrap();
        let r = GuardedWriter::new(
            &game.join("Mods/p"),
            std::slice::from_ref(&game),
            base.join("b"),
            None,
            Arc::new(FakeClock::new(1)),
        );
        assert!(matches!(r, Err(ToolkitError::PathRefused { .. })));
    }

    #[cfg(unix)]
    #[test]
    fn a_link_that_leaves_the_root_is_refused() {
        let (_t, base) = temp();
        let outside = base.join("outside");
        std::fs::create_dir_all(outside.as_std_path()).unwrap();
        let link = base.join("proj/link");
        if std::os::unix::fs::symlink(outside.as_std_path(), link.as_std_path()).is_err() {
            return;
        }
        let (w, _) = writer(&base.join("proj"), &[]);
        assert!(matches!(
            w.write("link/x.xml", "x", false),
            Err(ToolkitError::PathRefused { .. })
        ));
        assert!(!outside.join("x.xml").as_std_path().exists());
    }
}
