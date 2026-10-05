//! Errors and diagnostic codes of the library crate.
//!
//! Scan problems in user content are [`Diagnostic`](rimstudio_core::diag::Diagnostic)s with the
//! codes in [`codes`]; they never abort a scan. The typed errors here are for operations that can
//! fail as a whole: reading or writing the manifest, or a source that cannot be listed.

use camino::Utf8PathBuf;
use thiserror::Error;

/// Diagnostic codes produced by the scanner, the source classifier and the duplicate resolver.
pub mod codes {
    use rimstudio_core::diag::DiagCode;

    /// A folder looked like a mod but has no readable `About.xml`.
    pub const NO_ABOUT: DiagCode = DiagCode::new("scan.no-about");
    /// The `About` folder is not spelled `About`, so the game may not see it on a case sensitive file system.
    pub const ABOUT_DIR_CASE: DiagCode = DiagCode::new("scan.about-dir-case");
    /// `About.xml` could not be read from disk.
    pub const ABOUT_UNREADABLE: DiagCode = DiagCode::new("scan.about-unreadable");
    /// `About.xml` could not be parsed at all; the mod is listed with defaults.
    pub const ABOUT_UNPARSED: DiagCode = DiagCode::new("scan.about-unparsed");
    /// `About.xml` has no usable `packageId`; a stand-in id was made up.
    pub const PACKAGE_ID_MISSING: DiagCode = DiagCode::new("scan.package-id-missing");
    /// `LoadFolders.xml` could not be read or parsed.
    pub const LOAD_FOLDERS_UNREADABLE: DiagCode = DiagCode::new("scan.load-folders-unreadable");
    /// A `LoadFolders.xml` entry points outside the mod folder and is ignored.
    pub const LOAD_FOLDER_OUTSIDE: DiagCode = DiagCode::new("scan.load-folder-outside");
    /// A definition file ended early or could not be read.
    pub const DEF_FILE_BROKEN: DiagCode = DiagCode::new("scan.def-file-broken");
    /// A folder could not be listed (permission, vanished, I/O error).
    pub const DIR_UNREADABLE: DiagCode = DiagCode::new("scan.dir-unreadable");
    /// A name that is not valid UTF-8 was skipped.
    pub const NON_UTF8_PATH: DiagCode = DiagCode::new("scan.non-utf8-path");
    /// A link below a source root was not followed.
    pub const LINK_SKIPPED: DiagCode = DiagCode::new("scan.link-skipped");
    /// A link points nowhere (or loops).
    pub const DANGLING_LINK: DiagCode = DiagCode::new("scan.dangling-link");
    /// A source folder is missing or not a folder; its cached mods stay listed as unavailable.
    pub const SOURCE_OFFLINE: DiagCode = DiagCode::new("deploy.source-offline");
    /// The same folder was reached through two sources and is listed once.
    pub const SAME_FOLDER: DiagCode = DiagCode::new("scan.same-folder");
    /// The scan was cancelled before it finished.
    pub const CANCELLED: DiagCode = DiagCode::new("job.cancelled");
    /// A new source overlaps the game `Mods` folder.
    pub const SOURCE_INSIDE_MODS: DiagCode = DiagCode::new("deploy.source-inside-mods");
    /// A new source overlaps another source.
    pub const SOURCE_OVERLAP: DiagCode = DiagCode::new("deploy.source-overlap");
    /// A folder to add is not a folder.
    pub const PATH_NOT_DIRECTORY: DiagCode = DiagCode::new("scan.path-not-directory");

    /// Every code defined here, for tests and documentation tooling.
    pub const ALL: [DiagCode; 18] = [
        NO_ABOUT,
        ABOUT_DIR_CASE,
        ABOUT_UNREADABLE,
        ABOUT_UNPARSED,
        PACKAGE_ID_MISSING,
        LOAD_FOLDERS_UNREADABLE,
        LOAD_FOLDER_OUTSIDE,
        DEF_FILE_BROKEN,
        DIR_UNREADABLE,
        NON_UTF8_PATH,
        LINK_SKIPPED,
        DANGLING_LINK,
        SOURCE_OFFLINE,
        SAME_FOLDER,
        CANCELLED,
        SOURCE_INSIDE_MODS,
        SOURCE_OVERLAP,
        PATH_NOT_DIRECTORY,
    ];
}

/// Why a scan operation failed as a whole.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ScanError {
    /// The worker pool could not be created.
    #[error("could not start scan workers: {0}")]
    Pool(String),
    /// A source folder could not be listed.
    #[error("source {path} cannot be read: {message}")]
    SourceUnreadable {
        /// The folder.
        path: Utf8PathBuf,
        /// The system message.
        message: String,
    },
}

/// Why the manifest could not be stored.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CacheError {
    /// The file could not be written.
    #[error("manifest {path} cannot be written: {message}")]
    Write {
        /// The manifest path.
        path: Utf8PathBuf,
        /// The system message.
        message: String,
    },
    /// The manifest could not be turned into JSON.
    #[error("manifest cannot be serialised: {0}")]
    Serialize(String),
}

/// The error type of the crate.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LibraryError {
    /// A scan failed as a whole.
    #[error(transparent)]
    Scan(#[from] ScanError),
    /// The manifest could not be stored.
    #[error(transparent)]
    Cache(#[from] CacheError),
}

impl ScanError {
    /// The stable error code.
    pub fn code(&self) -> &'static str {
        match self {
            ScanError::Pool(_) => "scan.pool",
            ScanError::SourceUnreadable { .. } => "scan.source-unreadable",
        }
    }
}

impl CacheError {
    /// The stable error code.
    pub fn code(&self) -> &'static str {
        match self {
            CacheError::Write { .. } => "cache.write",
            CacheError::Serialize(_) => "cache.serialize",
        }
    }
}

impl LibraryError {
    /// The stable error code of the wrapped error.
    pub fn code(&self) -> &'static str {
        match self {
            LibraryError::Scan(e) => e.code(),
            LibraryError::Cache(e) => e.code(),
        }
    }
}

/// Result alias of the crate.
pub type LibraryResult<T> = Result<T, LibraryError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_code_follows_the_grammar_and_is_unique() {
        let mut seen = std::collections::BTreeSet::new();
        for code in codes::ALL {
            assert!(code.is_well_formed(), "{}", code.as_str());
            assert!(seen.insert(code.as_str().to_owned()), "{}", code.as_str());
        }
    }

    #[test]
    fn errors_have_stable_codes() {
        let e: LibraryError = ScanError::Pool("x".into()).into();
        assert_eq!(e.code(), "scan.pool");
        let e: LibraryError = CacheError::Serialize("x".into()).into();
        assert_eq!(e.code(), "cache.serialize");
    }
}
