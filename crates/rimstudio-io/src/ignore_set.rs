//! Ignore pattern sets (deferred, not part of 0.1.0).
//!
//! The design calls for `IgnoreSet::from_patterns(&[String])`, a compiled set of gitignore style
//! patterns (through the `ignore` crate) that scans and the Workshop publisher use to leave out
//! build output, editor files and version control folders. The 0.1.0 slice has no publisher and
//! scans prune with the closure of [`crate::walk::ScanOptions::prune`], so this module is
//! intentionally empty until the publisher milestone.
