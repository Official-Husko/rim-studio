//! The header of a save file: which game version and mods it was made with.
//!
//! A `.rws` save can be many megabytes, but its `meta` element comes first. [`read_meta`] reads
//! from a stream only until `</meta>` is found, then parses what it has read.

use std::io::Read;

use crate::about::normalise_encoding;
use crate::error::{XmlError, XmlResult};
use crate::modes::ParseMode;
use crate::reader::parse_document;
use rimstudio_core::tree::Node;

/// What a save's `meta` element says.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SaveMeta {
    /// `gameVersion`, for example `1.6.4871 rev598`.
    pub game_version: Option<String>,
    /// `modIds`: the package ids of the mods the save was made with, in load order.
    pub mod_ids: Vec<String>,
    /// `modSteamIds`: the Workshop ids, parallel to `mod_ids` (0 or empty for non Workshop mods).
    pub mod_steam_ids: Vec<String>,
    /// `modNames`: the display names, parallel to `mod_ids`.
    pub mod_names: Vec<String>,
}

/// The most bytes read while looking for the end of the `meta` element.
pub const MAX_META_BYTES: usize = 4 * 1024 * 1024;

fn items(meta: &Node, tag: &str) -> Vec<String> {
    meta.child(tag)
        .map(|n| {
            n.elements()
                .map(|li| li.text_content().trim().to_owned())
                .collect()
        })
        .unwrap_or_default()
}

fn find(haystack: &[u8], needle: &[u8], from: usize) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    let start = from.min(haystack.len());
    haystack[start..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + start)
}

/// Reads the `meta` element of a save from `reader`, stopping at `</meta>`.
///
/// # Errors
/// [`XmlError::Io`] when reading fails, [`XmlError::NoRoot`] or [`XmlError::Parse`] when no
/// `meta` element can be found within [`MAX_META_BYTES`].
pub fn read_meta(mut reader: impl Read) -> XmlResult<SaveMeta> {
    const END: &[u8] = b"</meta>";
    let mut buf: Vec<u8> = Vec::with_capacity(16 * 1024);
    let mut chunk = [0u8; 8192];
    let mut scanned = 0usize;
    let end = loop {
        if let Some(pos) = find(&buf, END, scanned) {
            break Some(pos + END.len());
        }
        scanned = buf.len().saturating_sub(END.len());
        if buf.len() >= MAX_META_BYTES {
            break None;
        }
        let n = reader.read(&mut chunk).map_err(|e| XmlError::Io {
            message: e.to_string(),
        })?;
        if n == 0 {
            break None;
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    if let Some(end) = end {
        buf.truncate(end);
    }
    let data = normalise_encoding(&buf);
    let doc = parse_document(&data, ParseMode::Tolerant)?;
    let meta = if doc.root.tag == "meta" {
        Some(&doc.root)
    } else {
        doc.root.child("meta")
    };
    let Some(meta) = meta else {
        return Err(XmlError::NoRoot);
    };
    Ok(SaveMeta {
        game_version: meta
            .child("gameVersion")
            .map(|n| n.text_content().trim().to_owned()),
        mod_ids: items(meta, "modIds"),
        mod_steam_ids: items(meta, "modSteamIds"),
        mod_names: items(meta, "modNames"),
    })
}
