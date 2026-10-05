//! Patch files (`Patches/**/*.xml`): the list of operations as node trees.
//!
//! Operations are returned as opaque nodes: this crate does not know what a `PatchOperation`
//! does. The patch engine interprets them.

use rimstudio_core::diag::Diagnostic;
use rimstudio_core::tree::Node;

use crate::error::XmlResult;
use crate::modes::ParseMode;
use crate::reader::parse_top_level;

/// A parsed patch file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchFile {
    /// The name of the root element (`Patch` in a healthy file).
    pub root_tag: String,
    /// The operations in file order.
    pub operations: Vec<Node>,
    /// Warnings found while reading.
    pub diagnostics: Vec<Diagnostic>,
}

/// Parses a patch file as the game's loader reads it and returns the operations below the root.
///
/// A root that is not named `Patch` gives a warning but the children are still returned, as in
/// the game.
///
/// # Errors
/// [`crate::XmlError`] for a file the game would skip (not well formed, DTD, no root).
pub fn parse_patch_file(bytes: &[u8]) -> XmlResult<Vec<Node>> {
    Ok(parse_patch_file_with(bytes, ParseMode::Game)?.operations)
}

/// [`parse_patch_file`] with an explicit mode and the warnings.
///
/// # Errors
/// As [`parse_patch_file`] (in tolerant mode only a file without root fails).
pub fn parse_patch_file_with(bytes: &[u8], mode: ParseMode) -> XmlResult<PatchFile> {
    let top = parse_top_level(bytes, mode, "Patch")?;
    Ok(PatchFile {
        root_tag: top.root_tag,
        operations: top.nodes,
        diagnostics: top.diagnostics,
    })
}
