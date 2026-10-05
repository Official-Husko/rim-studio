//! Pieces shared by the tool modules: a line diff, the guarded project writer, the reading of a project
//! folder and the merging of generated nodes into existing XML files.
//!
//! A tool module never uses another tool module, so everything that two tools need (the designer plans
//! writes into a project; the project tool creates one with the same writer) lives here, in a module that
//! belongs to no tool and is always compiled.
//!
//! - [`diff`]: a small in house unified line diff.
//! - [`writer`]: [`writer::GuardedWriter`], the only way the toolkit writes: a path must be a safe relative
//!   path, inside the project root and outside the protected folders; every replaced file is backed up and
//!   every written file is read back.
//! - [`projectfs`]: [`projectfs::ProjectView`], a read only view of a mod folder (About, `LoadFolders.xml`, the
//!   file layout of the project).
//! - [`merge`]: byte span merging of generated sections and `LoadFolders.xml` entries into existing files.

pub mod diff;
pub mod env;
pub mod merge;
pub mod projectfs;
pub mod writer;
